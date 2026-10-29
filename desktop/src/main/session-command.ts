/** Renderer session commands the main process may forward to the Host. */
const ACTIONS = new Set([
  'list_agents', 'workspace', 'load_roadmap', 'new_session', 'prompt', 'cancel', 'permission',
  'model', 'configure', 'authenticate', 'close', 'delete', 'search_context', 'resolve_context', 'attach_files',
]);
const METHODS = new Set([
  'settings.workbench', 'settings.workbench.get', 'settings.credentials', 'settings.credentials.get',
]);

export function acceptedSessionCommand(command: unknown): boolean {
  if (!command || typeof command !== 'object') return false;
  const record = command as { action?: unknown; method?: unknown };
  const action = typeof record.action === 'string' ? record.action : '';
  const method = typeof record.method === 'string' ? record.method : '';
  return ACTIONS.has(action) || METHODS.has(method);
}

type Slot = { id: string; label: string; configured: true; status: 'configured' };

function presentSlot(slot: unknown): Slot {
  if (!slot || typeof slot !== 'object') return { id: '', label: '', configured: true, status: 'configured' };
  const record = slot as Record<string, unknown>;
  return {
    id: typeof record.id === 'string' ? record.id : '',
    label: typeof record.label === 'string' ? record.label : '',
    configured: true,
    status: 'configured',
  };
}

/**
 * Public credential event for the renderer. Drops secret envelopes from the
 * payload and from `text` before the event can be shown or snapshotted.
 */
export function presentCredentialEvent<T extends object>(message: T): T {
  const record = message as { kind?: unknown; text?: unknown; data?: unknown };
  if (record.kind !== 'credential_settings') return message;
  const data = record.data;
  const credentials = data && typeof data === 'object' && Array.isArray((data as { credentials?: unknown }).credentials)
    ? (data as { credentials: unknown[] }).credentials.map(presentSlot)
    : [];
  return { ...message, text: '', data: { credentials } };
}
