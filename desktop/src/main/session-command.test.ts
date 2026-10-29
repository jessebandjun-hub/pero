import assert from 'node:assert/strict';
import { test } from 'node:test';
import { acceptedSessionCommand, presentCredentialEvent } from './session-command.ts';

const SECRET = 'c2VjcmV0LW1hcmtlci1kb25vdC1lY2hv';
const PLAINTEXT = 'secret-marker-donot-echo';

test('renderer session IPC forwards credential settings and existing workbench commands', () => {
  assert.equal(acceptedSessionCommand({ method: 'settings.credentials.get' }), true);
  assert.equal(acceptedSessionCommand({ method: 'settings.credentials', operation: 'replace', id: 'github', label: 'GitHub', secret: SECRET }), true);
  assert.equal(acceptedSessionCommand({ method: 'settings.credentials', operation: 'remove', id: 'github' }), true);
  assert.equal(acceptedSessionCommand({ method: 'settings.workbench.get' }), true);
  assert.equal(acceptedSessionCommand({ method: 'settings.workbench', preferences: { appearance: 'dark' } }), true);
  assert.equal(acceptedSessionCommand({ action: 'list_agents' }), true);
  assert.equal(acceptedSessionCommand({ method: 'settings.remote', connection: { token: SECRET } }), false);
  assert.equal(acceptedSessionCommand({ action: 'read_file', path: '/tmp/secret' }), false);
  assert.equal(acceptedSessionCommand(null), false);
  assert.equal(acceptedSessionCommand('settings.credentials'), false);
});

test('credential settings events delivered to the renderer omit secret envelopes', () => {
  const event = presentCredentialEvent({
    version: 1,
    kind: 'credential_settings',
    request_id: null,
    text: SECRET,
    data: {
      secret: SECRET,
      credentials: [{ id: 'github', label: 'GitHub', configured: true, status: 'configured', secret: SECRET, token: PLAINTEXT }],
    },
  });
  const rendered = JSON.stringify(event);
  assert.equal(rendered.includes(SECRET), false);
  assert.equal(rendered.includes(PLAINTEXT), false);
  assert.deepEqual(event.data, { credentials: [{ id: 'github', label: 'GitHub', configured: true, status: 'configured' }] });
  assert.equal(event.text, '');

  const other = presentCredentialEvent({ kind: 'error', text: 'error.credentials', data: null });
  assert.equal(other.text, 'error.credentials');
});
