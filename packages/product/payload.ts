import { createHash } from 'node:crypto';

const slug = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const revision = /^[0-9a-f]{40}$/;

function relativePath(path: unknown): path is string {
  return typeof path === 'string' && path.length > 0 && !/[\\:\x00]/.test(path) &&
    path.split('/').every(part => part !== '' && part !== '.' && part !== '..');
}

function sha256(bytes: Uint8Array | string): string {
  return createHash('sha256').update(bytes).digest('hex');
}

export type PayloadDiff = { readonly missing: readonly string[]; readonly extra: readonly string[] };

/** Exact files a package may contain, plus the locks a receipt must cite. */
export class DeclaredPayload {
  readonly schemaVersion = 1;
  readonly name: string;
  readonly files: readonly string[];
  readonly dependencies: readonly { readonly name: string; readonly path: string }[];

  private constructor(name: string, files: string[], dependencies: { name: string; path: string }[]) {
    this.name = name;
    this.files = Object.freeze([...files]);
    this.dependencies = Object.freeze(dependencies.map(item => Object.freeze({ ...item })));
    Object.freeze(this);
  }

  static parse(value: unknown): DeclaredPayload {
    const v = value as { schemaVersion?: unknown; name?: unknown; files?: unknown; dependencies?: unknown };
    const files = v?.files;
    const dependencies = v?.dependencies;
    const sorted = (items: string[]) => items.every((item, index) => index === 0 || items[index - 1] < item);
    const dependencyOk = Array.isArray(dependencies) && dependencies.length > 0 && dependencies.every(item => {
      const entry = item as { name?: unknown; path?: unknown };
      return !!item && typeof item === 'object' && !Array.isArray(item) &&
        Object.keys(item).every(key => key === 'name' || key === 'path') &&
        typeof entry.name === 'string' && slug.test(entry.name) && relativePath(entry.path);
    });
    if (!v || typeof v !== 'object' || Array.isArray(v) ||
      Object.keys(v).some(key => !['schemaVersion', 'name', 'files', 'dependencies'].includes(key)) ||
      v.schemaVersion !== 1 || typeof v.name !== 'string' || !slug.test(v.name) ||
      !Array.isArray(files) || !files.length || !files.every(relativePath) || new Set(files).size !== files.length ||
      !sorted(files) || !dependencyOk ||
      new Set(dependencies.map((item: { name: string }) => item.name)).size !== dependencies.length ||
      !sorted(dependencies.map((item: { name: string }) => item.name))) {
      throw new Error('Invalid declared payload');
    }
    return new DeclaredPayload(v.name, files, dependencies);
  }

  /** Missing and extra are sorted. A duplicate listing is not a payload. */
  diff(actual: readonly string[]): PayloadDiff {
    if (!actual.every(path => typeof path === 'string') || new Set(actual).size !== actual.length) {
      throw new Error('Invalid payload listing');
    }
    const declared = new Set(this.files);
    const seen = new Set(actual);
    return {
      missing: this.files.filter(path => !seen.has(path)),
      extra: Object.freeze([...actual].filter(path => !declared.has(path)).sort()),
    };
  }

  /**
   * Bind this declaration to a source revision, lock bytes, file bytes, and
   * archive bytes. payloadSha256 is the sha256 of each declared path, a NUL,
   * the hex sha256 of that file, and a newline, in declaration order.
   */
  receipt(input: {
    sourceRevision: string;
    dependencies: readonly { name: string; path: string; bytes: Uint8Array }[];
    files: readonly { path: string; bytes: Uint8Array }[];
    archive: Uint8Array;
  }) {
    const keys = input && typeof input === 'object' ? Object.keys(input) : [];
    const sameIdentity = input?.dependencies?.length === this.dependencies.length &&
      input.dependencies.every((item, index) => item?.name === this.dependencies[index].name &&
        item.path === this.dependencies[index].path && Object.keys(item).every(key => ['name', 'path', 'bytes'].includes(key)) &&
        item.bytes instanceof Uint8Array);
    const sameFiles = input?.files?.length === this.files.length &&
      input.files.every((item, index) => item?.path === this.files[index] &&
        Object.keys(item).every(key => key === 'path' || key === 'bytes') && item.bytes instanceof Uint8Array);
    if (keys.length !== 4 || keys.some(key => !['sourceRevision', 'dependencies', 'files', 'archive'].includes(key)) ||
      !revision.test(input?.sourceRevision || '') || !sameIdentity || !sameFiles ||
      !(input.archive instanceof Uint8Array) || input.archive.length === 0) {
      throw new Error('Payload provenance does not match the declaration');
    }
    const fileDigests = input.files.map(file => sha256(file.bytes));
    const preimage = createHash('sha256');
    input.files.forEach((file, index) => {
      preimage.update(file.path);
      preimage.update('\0');
      preimage.update(fileDigests[index]);
      preimage.update('\n');
    });
    const dependencies = Object.freeze(input.dependencies.map(item => Object.freeze({
      name: item.name, path: item.path, sha256: sha256(item.bytes),
    })));
    return Object.freeze({
      schemaVersion: 1 as const,
      name: this.name,
      sourceRevision: input.sourceRevision,
      dependencies,
      payloadSha256: preimage.digest('hex'),
      archiveSha256: sha256(input.archive),
      files: this.files,
    });
  }
}
