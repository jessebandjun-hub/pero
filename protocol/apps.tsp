import "@typespec/json-schema";
using JsonSchema;
@jsonSchema
namespace Pero.Apps;

model AppClosed { ...Record<never>; }
@minLength(1) @maxLength(64) @pattern("^[a-z0-9]+(?:-[a-z0-9]+)*$") scalar AppName extends string;
@minLength(1) @maxLength(128) scalar RequestId extends string;

model AppPanel { id: AppName; title: string; entry: string; ...AppClosed; }
/** Declared host permission requirements. This host grants none, so installed records publish an empty list. */
model AppPermissions { host: string[]; ...AppClosed; }
/** Runtime the App needs. `none` is a panel with no backend. */
model AppEnvironment { runtime: "none" | "python" | "node"; ...AppClosed; }
/** Public catalog: no local paths, backend credentials, or grants. */
model AppRecord {
  name: AppName;
  version: string;
  displayName: string;
  description: string;
  enabled: boolean;
  panels: AppPanel[];
  permissions: AppPermissions;
  environment: AppEnvironment;
  ...AppClosed;
}
/** Host-to-main-process descriptor. Never forwarded to App or Workbench renderers. */
model BackendAccess { url: string; token: string; ...AppClosed; }
model AppAccess { root: string; entry: string; backend?: BackendAccess; ...AppClosed; }
model RequestBase { method: "apps"; id: RequestId; ...AppClosed; }
model ListApps { action: "list"; ...RequestBase; }
model InstallApp { action: "install"; path: string; ...RequestBase; }
model EnableApp { action: "enable"; name: AppName; ...RequestBase; }
model DisableApp { action: "disable"; name: AppName; ...RequestBase; }
model UninstallApp { action: "uninstall"; name: AppName; ...RequestBase; }
model ResolveApp { action: "resolve"; name: AppName; ...RequestBase; }
@oneOf union AppRequest { ListApps, InstallApp, EnableApp, DisableApp, UninstallApp, ResolveApp }
model AppSuccess { ok: true; value: AppRecord[] | AppAccess; ...AppClosed; }
model AppFailure { ok: false; error: string; ...AppClosed; }
@oneOf union AppResult { AppSuccess, AppFailure }
