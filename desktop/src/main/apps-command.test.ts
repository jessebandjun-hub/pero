import assert from 'node:assert/strict';
import { mock, test } from 'node:test';

mock.module('electron', {
  namedExports: { net: { fetch: async () => { throw new Error('unused'); } } },
});

const { DesktopApps } = await import('./apps.ts');

type Event = { version: number; kind: string; request_id: string | null; text: string; data?: unknown };
type Request = { method?: string; action?: string; id?: string; names?: string[]; name?: string };

function harness() {
  const written: Request[] = [];
  const sent: Event[] = [];
  const apps = new DesktopApps(
    request => written.push(request as Request),
    event => sent.push(event),
    async () => undefined,
  );
  return { apps, written, sent };
}

const catalog = [{
  name: 'gamma', version: '1', displayName: 'Gamma', description: '', enabled: false, panels: [],
}];

test('reorder is sent on the apps channel and a saved catalog is published', async () => {
  const { apps, written, sent } = harness();
  try {
    const pending = apps.command({ action: 'apps_reorder', names: ['gamma', 'alpha', 'beta'] });
    assert.equal(written.length, 1);
    assert.equal(written[0].method, 'apps');
    assert.equal(written[0].action, 'reorder');
    assert.deepEqual(written[0].names, ['gamma', 'alpha', 'beta']);
    assert.equal(written[0].name, undefined);
    apps.accept({
      version: 1, kind: 'apps_result', request_id: written[0].id!, text: '',
      data: { ok: true, value: catalog },
    });
    await pending;
    assert.equal(sent.length, 1);
    assert.equal(sent[0].kind, 'apps_catalog');
    assert.deepEqual(sent[0].data, catalog);
  } finally {
    apps.close();
  }
});

test('a name that is not an App name is refused before the Host is asked', async () => {
  const { apps, written, sent } = harness();
  try {
    await apps.command({ action: 'reorder', names: ['Nope'] });
    await apps.command({ action: 'apps_reorder', names: 'gamma' });
    await apps.command({ action: 'apps_resolve', name: 'gamma' });
    assert.deepEqual(written, []);
    assert.deepEqual(sent.map(event => event.text), [
      'Invalid catalog order',
      'Invalid catalog order',
      'Unsupported App action',
    ]);
    assert.equal(sent.every(event => event.kind === 'apps_error'), true);
  } finally {
    apps.close();
  }
});

test('a Host rejection is shown and the catalog event is not published', async () => {
  const { apps, written, sent } = harness();
  try {
    const pending = apps.command({ action: 'reorder', names: ['alpha', 'alpha'] });
    assert.deepEqual(written[0].names, ['alpha', 'alpha']);
    apps.accept({
      version: 1, kind: 'apps_result', request_id: written[0].id!, text: '',
      data: { ok: false, error: 'Catalog order lists an App more than once' },
    });
    await pending;
    assert.equal(written.length, 1);
    assert.equal(sent.length, 1);
    assert.equal(sent[0].kind, 'apps_error');
    assert.equal(sent[0].text, 'Catalog order lists an App more than once');
  } finally {
    apps.close();
  }
});
