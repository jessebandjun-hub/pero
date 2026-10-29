// One real stdio wiring check for App install, startup recovery and private resolution.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { mkdtemp, mkdir, writeFile, rm, realpath } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';
import Ajv2020 from 'ajv/dist/2020.js';
import schema from '../../protocol/generated/host.schema.json' with { type: 'json' };

const root = resolve(import.meta.dirname, '../..');
const temp = await mkdtemp(resolve(tmpdir(), 'pero-app-wire-'));
const source = resolve(temp, 'sample');
await mkdir(resolve(source, 'dist'), { recursive: true });
await writeFile(resolve(source, 'dist/index.html'), '<h1>Sample App</h1>');
await writeFile(resolve(source, 'app.json'), JSON.stringify({
  schemaVersion: 1, name: 'sample', version: '0.1.0', displayName: 'Sample App',
  contributes: { panels: [{ id: 'main', title: 'Sample', entry: 'dist/index.html' }] },
}));
const ajv = new Ajv2020({ strict: true, strictTypes: false });
ajv.addSchema(schema);
const validateInput = ajv.getSchema(`${schema.$id}#/$defs/AppRequest`);
const validateResult = ajv.getSchema(`${schema.$id}#/$defs/AppsResultEvent`);
const failures = [];
const launch = async () => {
  const child = spawn(resolve(root, 'target/debug/pero-host'), ['--stdio'], {
    cwd: temp, stdio: ['pipe', 'pipe', 'inherit'],
    env: { ...process.env, PERO_SETTINGS_PATH: resolve(temp, 'settings.json'),
      PERO_APPS_ROOT: resolve(temp, 'installed'), PERO_BUILTIN_APPS_ROOT: '' },
  });
  const pending = new Map();
  let ready;
  const started = new Promise(resolveReady => { ready = resolveReady; });
  createInterface({ input: child.stdout }).on('line', line => {
    const event = JSON.parse(line);
    if (event.kind === 'ready') ready();
    if (event.kind !== 'apps_result') return;
    if (!validateResult(event)) failures.push(ajv.errorsText(validateResult.errors));
    pending.get(event.request_id)?.(event.data);
  });
  await Promise.race([started, new Promise((_, reject) => {
    const timer = setTimeout(() => reject(new Error('Host readiness timeout')), 10000); timer.unref();
  })]);
  return {
    request: async (action, fields = {}) => {
      const input = { method: 'apps', id: crypto.randomUUID(), action, ...fields };
      assert(validateInput(input), ajv.errorsText(validateInput.errors));
      return new Promise((resolveReply, reject) => {
        const timer = setTimeout(() => reject(new Error(`App ${action} timeout`)), 10000);
        pending.set(input.id, result => { clearTimeout(timer); pending.delete(input.id); resolveReply(result); });
        child.stdin.write(`${JSON.stringify(input)}\n`);
      });
    },
    close: () => new Promise(done => { child.once('exit', done); child.stdin.end(); }),
  };
};
let host;
try {
  host = await launch();
  assert.deepEqual(await host.request('list'), { ok: true, value: [] });
  const installed = await host.request('install', { path: source });
  assert.equal(installed.value[0].name, 'sample');
  assert.equal(installed.value[0].enabled, false);
  assert.deepEqual(installed.value[0].permissions, { host: [] });
  assert.deepEqual(installed.value[0].environment, { runtime: 'none' });
  const declared = resolve(temp, 'declared');
  await mkdir(resolve(declared, 'dist'), { recursive: true });
  await mkdir(resolve(declared, 'backend'), { recursive: true });
  await writeFile(resolve(declared, 'dist/index.html'), '<h1>Declared</h1>');
  await writeFile(resolve(declared, 'backend/server.py'), '# unused\n');
  await writeFile(resolve(declared, 'app.json'), JSON.stringify({
    schemaVersion: 1, name: 'declared', version: '0.2.0', displayName: 'Declared',
    contributes: { panels: [{ id: 'main', title: 'Declared', entry: 'dist/index.html' }] },
    environment: { runtime: 'python' },
    backend: { type: 'python', entry: 'backend/server.py' },
  }));
  const listed = await host.request('install', { path: declared });
  const declaredRecord = listed.value.find(app => app.name === 'declared');
  assert.deepEqual(declaredRecord.permissions, { host: [] });
  assert.deepEqual(declaredRecord.environment, { runtime: 'python' });
  const mismatch = resolve(temp, 'mismatch');
  await mkdir(resolve(mismatch, 'dist'), { recursive: true });
  await mkdir(resolve(mismatch, 'backend'), { recursive: true });
  await writeFile(resolve(mismatch, 'dist/index.html'), '<h1>Mismatch</h1>');
  await writeFile(resolve(mismatch, 'backend/server.py'), '# unused\n');
  await writeFile(resolve(mismatch, 'app.json'), JSON.stringify({
    schemaVersion: 1, name: 'mismatch', version: '0.1.0', displayName: 'Mismatch',
    contributes: { panels: [{ id: 'main', title: 'Mismatch', entry: 'dist/index.html' }] },
    environment: { runtime: 'node' },
    backend: { type: 'python', entry: 'backend/server.py' },
  }));
  const refused = await host.request('install', { path: mismatch });
  assert.equal(refused.ok, false);
  assert.equal((await host.request('uninstall', { name: 'declared' })).value.length, 1);
  assert.equal((await host.request('enable', { name: 'sample' })).ok, true);
  await host.close();
  host = await launch();
  assert.equal((await host.request('list')).value[0].enabled, true);
  const access = await host.request('resolve', { name: 'sample' });
  assert.equal(access.value.entry, 'dist/index.html');
  assert.equal(access.value.root, await realpath(resolve(temp, 'installed/sample')));
  assert.equal((await host.request('uninstall', { name: 'sample' })).value.length, 0);
  assert.deepEqual(failures, []);
  console.log('App stdio wiring and generated contract passed.');
} finally {
  if (host) await host.close();
  await rm(temp, { recursive: true, force: true });
}
