/** Renderer App actions the main process may send to the Host as `method: "apps"`. */
const ACTIONS = new Set([
  'list', 'install', 'enable', 'disable', 'uninstall', 'open', 'reorder',
]);

const APP_NAME = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

export function acceptedAppsAction(action: string | undefined): boolean {
  return ACTIONS.has(action || '');
}

/** Same App name rule as the Host contract. Not a catalog-permutation check. */
export function appName(name: unknown): name is string {
  return typeof name === 'string' && name.length <= 64 && APP_NAME.test(name);
}

/**
 * Entry check for `apps` `reorder`. Each item must be an App name.
 * The Host still rejects a repeated, unknown, or incomplete permutation.
 */
export function reorderNames(names: unknown): string[] {
  if (!Array.isArray(names) || names.some(name => !appName(name))) throw new Error('Invalid catalog order');
  return names;
}
