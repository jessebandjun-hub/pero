---
kind: verification_report
date: 2026-10-29
---

# App 数据迁移 acceptance

ROADMAP item 「App 数据迁移」 stays unchecked. Acceptance is that records can be
checked before and after an App version upgrade, and that a failed migration
does not overwrite recoverable old data. This note records the Host slice on
top of the specified-version update branch, and the Desktop update that
surfaces that result. It does not mark the item complete.

## Landed

`apps` `update` still publishes a package only when the caller names the
installed App and that package's version. Before the package directory moves,
the Host migrates domain files under `app-data/<name>` or leaves them in place.

The incoming manifest may declare `migrations`: one direct step whose `from`
is the installed version and whose `to` is the requested version. Each `rename`
entry moves one domain file. Files the step does not name stay on their
existing paths. The file bytes are not rewritten. A step that does not change
any path, and an update with no matching declaration, keep the directory and
write a receipt when the directory already exists.

The receipt is `app-data/<name>/.pero-migration.json`. It lists every migrated
record's path before and after, plus the SHA-256 of the bytes. After a rename,
the new file's digest matches the receipt and the old path is gone.

A migration that cannot be applied (a declared path outside the domain
directory, two records landing on one path, or a symlink in the domain
directory) does not publish the new package. The previous files stay readable.
The update error names the App, the version kept, the domain directory, and
`app-data/<name>/.pero-migration-error.json`, which stores `from`, `to`, and
the same reason.

The new data tree is staged and swapped before the package directory moves.
Opening the store deletes a leftover `.staging-<name>` directory. A
`.rollback-<name>` data backup is restored when the installed package version
is not the receipt's `to` version, and removed when that publish finished.

## Evidence

Host and runtime, L0. Observed on 2026-10-29 with Rust 1.99.1. `pero-runtime`
and `pero-host` do not link gpui-kit, so this run used Cargo directly:

- `cargo test -p pero-runtime --offline --lib apps` — 10 passed, including
  `migration_records_stay_checkable`,
  `update_migrates_domain_records_and_keeps_them_when_migration_fails`, and
  `interrupted_data_migration_restores_previous_records`.
- `cargo build -p pero-host --offline` then `node tests/contracts/apps.mjs` —
  App stdio wiring passed. The same host updates `draft.json` to
  `records/draft.json`, checks the receipt digest, and a colliding rename
  keeps both previous files plus `.pero-migration-error.json`.

`node scripts/cargo.mjs` also runs these Cargo commands and needs
`PERO_GPUI_KIT_PATH`. That checkout was not present here. `--locked` was not
used: this Cargo also wants to record registry sources for the GPUI packages
that the committed lock leaves unresolved until the path patch is applied.

## Desktop

Desktop sends the same `apps` `update` as the Host slice. It does not add a
migration API. The catalog version updates when Host accepts the package.
When Host rejects the migration, the previous catalog version and selection
stay on screen, and the banner shows the Host error. That error names the
App, the version kept, the domain directory, and
`app-data/<name>/.pero-migration-error.json` when Host could write the
pointer. Desktop does not read or render that file.

The command checks and the catalog-version behavior are the same run
recorded in [apps-update.md](apps-update.md). No Electron process was
launched.

## Still open

No Desktop session checked records before and after an upgrade, and no
packaged Electron build was tested. The ROADMAP checkbox stays open.

Only one direct `from`/`to` step is applied. A jump across intermediate
declarations keeps the previous files instead of chaining those steps. The
migration renames files and preserves their bytes; it does not rewrite record
contents. A transforming publish rebuilds the domain directory from non-dot
files and the receipt, so dotfiles in that directory are not carried into the
new tree. An update does not stop a running backend until the package and the
data publish both succeed, so a backend can still write during the snapshot.
There is no marketplace or automatic update service. Desktop does not browse
domain files.
