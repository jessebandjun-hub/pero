import { test } from 'node:test';
import assert from 'node:assert/strict';
import { prepareAppCommand } from './app-command.ts';

test('Desktop accepts an App update only with a target version', () => {
  const version = '0.2.0';
  const long = 'v'.repeat(64);
  for (const [label, value, expected] of [
    ['list', { action: 'apps_list' }, { type: 'request', action: 'list', fields: {} }],
    ['install', { action: 'install' }, { type: 'install' }],
    ['enable', { action: 'apps_enable', name: 'virtual-cell' }, { type: 'request', action: 'enable', fields: { name: 'virtual-cell' } }],
    ['disable', { action: 'disable', name: 'virtual-cell' }, { type: 'request', action: 'disable', fields: { name: 'virtual-cell' } }],
    ['uninstall', { action: 'uninstall', name: 'virtual-cell' }, { type: 'request', action: 'uninstall', fields: { name: 'virtual-cell' } }],
    ['open', { action: 'open', name: 'virtual-cell', panel: 'main' }, { type: 'open', name: 'virtual-cell', panel: 'main' }],
    ['update', { action: 'apps_update', name: 'virtual-cell', version: ` ${version} ` }, { type: 'update', name: 'virtual-cell', version }],
    ['update at the length limit', { action: 'update', name: 'virtual-cell', version: long }, { type: 'update', name: 'virtual-cell', version: long }],
  ] as const) {
    assert.deepEqual(prepareAppCommand(value), expected, label);
  }
  for (const [label, value, message] of [
    ['unknown action', { action: 'recover' }, 'Unsupported App action'],
    ['missing app', { action: 'update', version: version }, 'Choose an App first'],
    ['bad name', { action: 'update', name: '../cell', version }, 'Invalid App name'],
    ['missing version', { action: 'update', name: 'virtual-cell' }, 'Enter a target version of 1–64 characters'],
    ['blank version', { action: 'apps_update', name: 'virtual-cell', version: '  ' }, 'Enter a target version of 1–64 characters'],
    ['version past the limit', { action: 'update', name: 'virtual-cell', version: `${long}x` }, 'Enter a target version of 1–64 characters'],
    ['numeric version', { action: 'update', name: 'virtual-cell', version: 2 }, 'Enter a target version of 1–64 characters'],
    ['enable without an app', { action: 'enable' }, 'Choose an App first'],
  ] as const) {
    assert.throws(() => prepareAppCommand(value), { message }, label);
  }
});
