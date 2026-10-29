---
kind: verification_report
date: 2026-10-29
---

# Plugin settings

ROADMAP item 「插件设置页」 stays unchecked. This slice is the Host catalog
the Desktop page will list and toggle. It is L0 evidence on top of `master`
`604cca1`. It does not mark the item complete. 「插件管理」 (install, update,
and remove) stays a later slice.

## Stored catalog

`PluginCatalog::apply` is the only enable and disable transition. Each success
is the catalog written to the Host settings file. A rejected change returns no
next catalog and does not write. Reading a missing or invalid `plugins` value
yields an empty catalog and does not create or rewrite the file.

There is no install, update, or remove command. Version is a field on a record
that is already stored. Enable and disable do not change it. A package update
does not exist on the Host yet, so this slice does not add one.

`PluginSettings` is the page state. `shown` is the stored version and enabled
flag. `show_effective` replaces that catalog only when the Host returns a valid
one. A `plugin_settings_error` carries no catalog, so the page must not call
`show_effective` for it. The previous rows stay, and the same enable or disable
can be sent again.

Host methods:

- `settings.plugins.get`
- `settings.plugins.enable`
- `settings.plugins.disable`

Success event: `plugin_settings`, `data` is the stored catalog (`name`,
`version`, `enabled`). Failure event: `plugin_settings_error` with
`error.plugins`.

## Verification

Rust 1.99.1. The workspace uses edition 2024.

- `cargo test -p pero-contracts -p pero-runtime --lib plugin` passed:
  enable and disable return the stored catalog and keep version; rejected
  edits leave it unchanged; a failed save keeps the shown plugin; a later
  success replaces it; a rejected settings write leaves the file, and a
  non-object file is not replaced.
- `cargo build -p pero-host` then `node tests/contracts/host.mjs` passed
  against `pero-host --stdio`. Get, disable, and enable each matched the
  file. An invalid stored catalog was reported empty and was not rewritten.
  A missing name, an unknown field, and a blank name emitted
  `plugin_settings_error` and left the stored catalog in place. Restart
  returned the same catalog. Locale and workbench preferences remained.
  The suite observed 34 of 49 declared event kinds; `plugin_settings` and
  `plugin_settings_error` were among the observed kinds.
- `node scripts/codegen/contracts.mjs --check` passed.
- `node scripts/check/locales.mjs` checked 524 bilingual messages.

## Open follow-ups

- Desktop settings page that lists each plugin's name, version, and enabled
  flag from `plugin_settings.data` through `PluginSettings`.
- Send enable and disable, then render the returned catalog. On
  `plugin_settings_error`, keep the previous list and leave the control
  available to retry.
- Empty, loading, error, and disabled states in the running workbench, plus
  an Electron session. That evidence is what can check 「插件设置页」.
- 「插件管理」 still owes install, update, and remove. Until a package update
  writes a new version into this catalog, the page has no Host update to
  reflect. After that write, `settings.plugins.get` is the list the page shows.
