export type CatalogApp = { name: string; enabled: boolean; version?: string };

export type FramePlan = {
  drop: string[];
  replaced: string[];
  versions: Map<string, string>;
};

/** Retained panels stay while the installed version is unchanged. */
export function framesToDrop(
  open: readonly { name: string }[],
  previousVersions: ReadonlyMap<string, string>,
  catalog: readonly CatalogApp[],
): FramePlan {
  const versions = new Map(previousVersions);
  const replaced: string[] = [];
  for (const app of catalog) {
    if (typeof app.version !== 'string') continue;
    const previous = versions.get(app.name);
    if (previous !== undefined && previous !== app.version) replaced.push(app.name);
    versions.set(app.name, app.version);
  }
  const drop = [...new Set(open.map(frame => frame.name).filter(name => {
    const record = catalog.find(app => app.name === name);
    return !record?.enabled || replaced.includes(name);
  }))];
  return { drop, replaced, versions };
}

/** A visible panel whose published version changed is opened again. */
export function reopenUpdatedApp(
  visible: boolean,
  active: { name?: string; panel?: string } | undefined,
  catalog: readonly CatalogApp[],
  replaced: readonly string[],
): { name: string; panel?: string } | undefined {
  if (!visible || !active?.name || !replaced.includes(active.name)) return undefined;
  if (!catalog.some(app => app.name === active.name && app.enabled)) return undefined;
  return { name: active.name, panel: active.panel };
}
