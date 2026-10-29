import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { DeclaredPayload } from './payload.ts';

const manifest = {
  schemaVersion: 1, name: 'app-sdk',
  files: ['README.md', 'package.json'],
  dependencies: [{ name: 'npm', path: 'package-lock.json' }],
};

test('declared payload accepts only a sorted exact file set', () => {
  for (const [label, value, accepted] of [
    ['app sdk shape', manifest, true],
    ['unsorted files', { ...manifest, files: ['package.json', 'README.md'] }, false],
    ['duplicate file', { ...manifest, files: ['README.md', 'README.md'] }, false],
    ['parent path', { ...manifest, files: ['../package.json'] }, false],
    ['absolute path', { ...manifest, files: ['/package.json'] }, false],
    ['windows path', { ...manifest, files: ['src\\index.tsx'] }, false],
    ['directory entry', { ...manifest, files: ['src/'] }, false],
    ['missing lock', { ...manifest, dependencies: [] }, false],
    ['unsorted lock', { ...manifest, dependencies: [{ name: 'npm', path: 'package-lock.json' }, { name: 'cargo', path: 'Cargo.lock' }] }, false],
    ['lock escape', { ...manifest, dependencies: [{ name: 'npm', path: '../package-lock.json' }] }, false],
    ['build command', { ...manifest, command: 'npm pack' }, false],
  ] as const) {
    if (accepted) assert.doesNotThrow(() => DeclaredPayload.parse(value), label);
    else assert.throws(() => DeclaredPayload.parse(value), /Invalid declared payload/, label);
  }
});

test('package listing must match the declared payload', () => {
  const payload = DeclaredPayload.parse(manifest);
  for (const [actual, missing, extra] of [
    [['README.md', 'package.json'], [], []],
    [['package.json', 'README.md'], [], []],
    [['README.md'], ['package.json'], []],
    [['README.md', 'package.json', '.env'], [], ['.env']],
    [['README.md', 'extra.txt'], ['package.json'], ['extra.txt']],
  ] as const) {
    assert.deepEqual(payload.diff(actual), { missing, extra });
  }
  assert.throws(() => payload.diff(['README.md', 'README.md']), /Invalid payload listing/);
});

test('provenance receipt traces source, lock bytes, file bytes, and the archive', () => {
  const payload = DeclaredPayload.parse(manifest);
  const input = {
    sourceRevision: 'a'.repeat(40),
    dependencies: [{ name: 'npm', path: 'package-lock.json', bytes: Buffer.from('lock') }],
    files: [
      { path: 'README.md', bytes: Buffer.from('readme') },
      { path: 'package.json', bytes: Buffer.from('{}') },
    ],
    archive: Buffer.from('archive-bytes'),
  };
  const receipt = payload.receipt(input);
  const fileHex = (bytes: string) => createHash('sha256').update(bytes).digest('hex');
  const preimage = `README.md\0${fileHex('readme')}\npackage.json\0${fileHex('{}')}\n`;
  assert.equal(receipt.sourceRevision, input.sourceRevision);
  assert.equal(receipt.dependencies[0].sha256, fileHex('lock'));
  assert.equal(receipt.payloadSha256, createHash('sha256').update(preimage).digest('hex'));
  assert.equal(receipt.archiveSha256, fileHex('archive-bytes'));
  assert.deepEqual(receipt.files, manifest.files);
  for (const [label, broken] of [
    ['short revision', { ...input, sourceRevision: 'abc' }],
    ['other lock', { ...input, dependencies: [{ name: 'cargo', path: 'Cargo.lock', bytes: Buffer.from('lock') }] }],
    ['missing file bytes', { ...input, files: input.files.slice(0, 1) }],
    ['empty archive', { ...input, archive: Buffer.alloc(0) }],
  ] as const) {
    assert.throws(() => payload.receipt(broken), /Payload provenance does not match the declaration/, label);
  }
});
