---
kind: verification_report
date: 2026-10-29
---

# App 更新 acceptance

ROADMAP item 「App 更新」 stays unchecked. Acceptance is a displayed version that
matches the version actually running, and a usable version after an update
fails. This note records the Host slice on top of `604cca1`. It does not mark
the item complete.

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

## Still open

Desktop `DesktopApps` still accepts only list, install, enable, disable,
uninstall, and open. The workbench catalog already renders `version`, and no
Desktop session was run for a successful update or a failed update that keeps
the previous package openable. There is no marketplace or automatic update
service. A declared direct data migration is recorded in
[apps-data-migration.md](apps-data-migration.md). Without that declaration,
an update leaves the previous data files in place.
