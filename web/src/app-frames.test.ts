import { test } from 'node:test';
import assert from 'node:assert/strict';
import { framesToDrop, reopenUpdatedApp } from './app-frames.ts';

const cell = (version: string, enabled = true) => ({ name: 'virtual-cell', enabled, version });

test('a new catalog version drops the retained panel and can reopen it', () => {
  const open = [{ name: 'virtual-cell' }, { name: 'notes' }];
  const first = framesToDrop(open, new Map(), [cell('0.1.0'), { name: 'notes', enabled: true, version: '1' }]);
  assert.deepEqual(first.drop, []);
  assert.deepEqual(first.replaced, []);
  const next = framesToDrop(open, first.versions, [cell('0.2.0'), { name: 'notes', enabled: true, version: '1' }]);
  assert.deepEqual(next.drop, ['virtual-cell']);
  assert.deepEqual(next.replaced, ['virtual-cell']);
  assert.equal(next.versions.get('virtual-cell'), '0.2.0');
  assert.deepEqual(reopenUpdatedApp(true, { name: 'virtual-cell', panel: 'main' }, [cell('0.2.0')], next.replaced), { name: 'virtual-cell', panel: 'main' });
  assert.equal(reopenUpdatedApp(false, { name: 'virtual-cell', panel: 'main' }, [cell('0.2.0')], next.replaced), undefined);
});

test('a failed catalog leaves the previous version and panel', () => {
  const previous = new Map([['virtual-cell', '0.1.0']]);
  const same = framesToDrop([{ name: 'virtual-cell' }], previous, [cell('0.1.0')]);
  assert.deepEqual(same.drop, []);
  const disabled = framesToDrop([{ name: 'virtual-cell' }], previous, [cell('0.1.0', false)]);
  assert.deepEqual(disabled.drop, ['virtual-cell']);
  assert.deepEqual(disabled.replaced, []);
  assert.equal(reopenUpdatedApp(true, { name: 'virtual-cell' }, [cell('0.1.0', false)], disabled.replaced), undefined);
  const removed = framesToDrop([{ name: 'virtual-cell' }], previous, []);
  assert.deepEqual(removed.drop, ['virtual-cell']);
  assert.equal(removed.versions.get('virtual-cell'), '0.1.0');
});
