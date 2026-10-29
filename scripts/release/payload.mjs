// Compare one declared payload to its directory and npm pack archive.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, relative, resolve, sep } from 'node:path';
import { DeclaredPayload } from '../../packages/product/payload.ts';

const manifestFlag = process.argv.indexOf('--manifest');
const manifestArg = process.argv[manifestFlag + 1];
if (process.argv[2] !== 'check' || manifestFlag < 0 || !manifestArg || manifestArg.startsWith('--')) {
  throw new Error('Use check --manifest <payload.json>');
}
const manifestPath = resolve(manifestArg);
const payloadRoot = dirname(manifestPath);
const declared = DeclaredPayload.parse(JSON.parse(readFileSync(manifestPath, 'utf8')));
const git = (cwd, args) => execFileSync('git', args, { cwd, encoding: 'utf8' }).trim();
const repo = realpathSync(git(payloadRoot, ['rev-parse', '--show-toplevel']));
const payloadReal = realpathSync(payloadRoot);
if (payloadReal !== repo && !payloadReal.startsWith(repo + sep)) throw new Error('Payload manifest is outside the repository');
const payloadRel = relative(repo, payloadReal);
const dirtySpec = [payloadRel, ...declared.dependencies.map(item => item.path)];
const dirty = git(repo, ['status', '--porcelain', '--untracked-files=all', '--', ...dirtySpec]);
if (dirty) throw new Error(`Declared payload inputs differ from HEAD:\n${dirty}`);

function listFiles(directory, prefix = '') {
  const found = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isSymbolicLink()) throw new Error(`Payload contains a symlink: ${path}`);
    if (entry.isDirectory()) found.push(...listFiles(join(directory, entry.name), path));
    else if (entry.isFile()) found.push(path);
    else throw new Error(`Payload contains a non-file: ${path}`);
  }
  return found;
}

function failIfDifferent(kind, diff) {
  if (!diff.missing.length && !diff.extra.length) return;
  throw new Error(`${kind} does not match the declared payload: missing ${diff.missing.join(', ') || 'none'}; extra ${diff.extra.join(', ') || 'none'}`);
}

const listed = listFiles(payloadReal);
failIfDifferent('Payload directory', declared.diff(listed));
const inside = path => {
  const real = realpathSync(path);
  if (real !== repo && !real.startsWith(repo + sep)) throw new Error(`Dependency path escapes the repository: ${path}`);
  return real;
};
const files = declared.files.map(path => {
  const real = realpathSync(resolve(payloadReal, path));
  if (!real.startsWith(payloadReal + sep) || !statSync(real).isFile()) throw new Error(`Declared payload file is missing: ${path}`);
  return { path, bytes: readFileSync(real) };
});
const dependencies = declared.dependencies.map(item => {
  const real = inside(resolve(repo, item.path));
  if (!statSync(real).isFile()) throw new Error(`Declared dependency is missing: ${item.path}`);
  return { name: item.name, path: item.path, bytes: readFileSync(real) };
});

const packedDir = mkdtempSync(join(tmpdir(), 'pero-payload-'));
try {
  const npmBin = process.platform === 'win32' ? 'npm.cmd' : 'npm';
  const stdout = execFileSync(npmBin, ['pack', '--json', '--ignore-scripts', '--pack-destination', packedDir], {
    cwd: payloadReal, encoding: 'utf8',
  });
  const start = stdout.indexOf('[');
  const end = stdout.lastIndexOf(']');
  if (start < 0 || end < start) throw new Error('npm pack returned no JSON');
  const parsed = JSON.parse(stdout.slice(start, end + 1));
  const pack = Array.isArray(parsed) ? parsed[0] : parsed;
  if (!pack || !Array.isArray(pack.files) || typeof pack.filename !== 'string') throw new Error('npm pack returned no archive file list');
  const members = pack.files.map(file => {
    const path = typeof file?.path === 'string' ? file.path : '';
    return path.startsWith('package/') ? path.slice('package/'.length) : path;
  }).filter(path => path && !path.endsWith('/'));
  failIfDifferent('npm pack archive', declared.diff(members));
  const archivePath = resolve(packedDir, basename(pack.filename));
  const receipt = declared.receipt({
    sourceRevision: git(repo, ['rev-parse', 'HEAD']),
    dependencies, files, archive: readFileSync(archivePath),
  });
  if (!/^[0-9a-f]{64}$/.test(receipt.payloadSha256) || !/^[0-9a-f]{64}$/.test(receipt.archiveSha256)) {
    throw new Error('Payload provenance digest is missing');
  }
  process.stdout.write(JSON.stringify(receipt, null, 2) + '\n');
} finally {
  rmSync(packedDir, { recursive: true, force: true });
}
