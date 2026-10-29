// Which App command Desktop will send. The folder path is chosen here in the shell, never taken from the renderer.

const actions = ['list', 'install', 'enable', 'disable', 'uninstall', 'open', 'update'];
const namePattern = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

type Command = { action?: string; name?: string; panel?: string; version?: string };

export type PreparedAppCommand =
  | { type: 'open'; name: string; panel?: string }
  | { type: 'install' }
  | { type: 'update'; name: string; version: string }
  | { type: 'request'; action: 'list' | 'enable' | 'disable' | 'uninstall'; fields: Record<string, string> };

export function prepareAppCommand(value: unknown): PreparedAppCommand {
  const command = (value ?? {}) as Command;
  const action = command.action?.replace(/^apps_/, '');
  if (!actions.includes(action || '')) throw new Error('Unsupported App action');
  if (command.name !== undefined && (command.name.length > 64 || !namePattern.test(command.name))) throw new Error('Invalid App name');
  if (action === 'open') {
    if (!command.name) throw new Error('Choose an App first');
    return { type: 'open', name: command.name, panel: command.panel };
  }
  if (action === 'install') return { type: 'install' };
  if (action === 'update') {
    if (!command.name) throw new Error('Choose an App first');
    const version = typeof command.version === 'string' ? command.version.trim() : '';
    if (!version || version.length > 64) throw new Error('Enter a target version of 1–64 characters');
    return { type: 'update', name: command.name, version };
  }
  if (action === 'list') return { type: 'request', action: 'list', fields: {} };
  if (!command.name) throw new Error('Choose an App first');
  return { type: 'request', action: action as 'enable' | 'disable' | 'uninstall', fields: { name: command.name } };
}
