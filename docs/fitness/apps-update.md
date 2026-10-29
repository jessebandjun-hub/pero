---
kind: verification_report
date: 2026-10-29
---

# App 更新 acceptance

ROADMAP item 「App 更新」 stays unchecked. Acceptance is a displayed version that
matches the version actually running, and a usable version after an update
fails. This note records the Host slice on top of `604cca1` and the
Desktop request that follows it. It does not mark the item complete.

## Landed

`apps` `update` takes an installed App name, a target version, and a local
package path. The package is published only when its identity is that App and
its manifest `version` equals the requested version. `list` reports the
version parsed from the installed manifest. `resolve` serves that same
directory. Domain data stays in the sibling `app-data/<name>` directory.

The new package is copied to a staging directory and its panel and backend
entries are checked before the installed directory moves. The previous
directory is parked at `.rollback-<name>` and restored when the staged
directory cannot be published. Opening the store restores that backup when
the published directory has no installation record, and removes the backup
when the published install is intact. A failed update returns an error that
names the App and the installed version kept. Preparation failures leave the
running backend in place. A successful update stops that backend so the next
`resolve` starts the package just published.

## Evidence

Host and runtime, L0. Observed on 2026-10-29 with Rust 1.99.1. `pero-runtime`
and `pero-host` do not link gpui-kit, so this run used Cargo directly:

- `cargo test -p pero-runtime --locked --lib apps` — 7 passed, including
  `update_replaces_only_the_specified_version` and
  `interrupted_update_recovery`.
- `cargo build -p pero-host --locked` then `node tests/contracts/apps.mjs` —
  App stdio wiring and the generated contract passed.
- `node scripts/codegen/contracts.mjs --check` — the generated host contract
  was current.

`node scripts/cargo.mjs` also runs these Cargo commands and needs
`PERO_GPUI_KIT_PATH`. That checkout was not present here.

The runtime test installs `0.1.0`, rejects a missing entry, a version
mismatch, a different package identity, and a symlink source, and checks that
the catalog version, the served HTML, the domain file, and the running
backend token stay on `0.1.0`. It then updates to requested `0.2.0` and checks
that the catalog version, the installed manifest, and the HTML `resolve`
serves are `0.2.0`, that the previous backend has stopped, and that the domain
file is unchanged. The contract test drives the same success and missing-entry
failure through `pero-host --stdio` and the generated `AppRequest` schema.

## Desktop

The workbench catalog already showed `version`. It now also sends an update.
The renderer asks for `apps_update` with the installed name and a target
version of 1–64 characters. `DesktopApps` allows that action, asks for a
package folder through the same directory dialog as install, and writes
`apps` `update` with `name`, `version`, and `path`. The path is not taken
from the renderer.

App commands use `pero:apps`. The Electron session IPC allowlist in
`desktop/src/main/index.ts` does not include `update`: a session command
with that action is still rejected, so it cannot skip the folder dialog or
the App request shape.

A successful update replaces the catalog. The row shows the version Host
returned, and the update form closes. A retained panel for that App is
dropped. If that panel is visible, Desktop opens it again so the next
`resolve` runs the published package. A failure does not replace the
catalog. The installed version, the selected panel, and the update form
stay, and the banner shows the Host error. Cancelling the folder dialog
leaves that banner in place.

## Desktop evidence

Desktop, L0, on 2026-10-29. No Electron process was launched.

- `node --test desktop/src/main/app-command.test.ts web/src/app-frames.test.ts`
  — the update action requires a target version; an unknown action is
  refused; a changed catalog version drops the retained panel and reopens
  it only while that panel is visible and the App stays enabled.
- `node scripts/check/locales.mjs` — 528 bilingual messages, including the
  new App update strings. `apps` is now part of the UI key check.
- `./node_modules/.bin/tsc --noEmit` — Desktop and web sources typechecked.
- `cargo +stable test -p pero-ui --lib apps` — 2 passed, including
  `failed_update_keeps_the_installed_version_visible`. Rust 1.99.1. This
  test does not link gpui-kit.
- `cargo +stable check -p pero-ui --features native` — the workbench
  compiled against crates.io gpui-kit 0.8.1. It then stopped on missing
  `web/public/fonts` binaries in `native.rs`. Those files are unrelated to
  this slice, and this run did not use the path-patched kit.

## Still open

No Desktop session was run, and no packaged Electron build was tested. A
displayed version was not compared with a package actually resolved after
an update, and a failed update was not opened again in the running app.
ROADMAP 「App 更新」 stays unchecked.

There is no marketplace or automatic update service. A declared direct data
migration is recorded in [apps-data-migration.md](apps-data-migration.md).
Without that declaration, an update leaves the previous data files in place.
