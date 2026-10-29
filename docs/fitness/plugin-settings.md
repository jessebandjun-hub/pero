---
kind: verification_report
date: 2026-10-29
---

# Plugin settings

ROADMAP item 「插件设置页」 stays unchecked. The Host catalog is
`cursor/plugin-settings-host-b461` (`81e5850`). This slice wires Desktop All
settings to that catalog. It does not mark the item complete. 「插件管理」
(install, update, and remove) stays a later slice.

No Electron session was run. Empty, loading, error, and disabled states were
not observed in a running workbench, so they cannot check the roadmap item.

## Desktop page

All settings sends `settings.plugins.get` and lists each plugin's name,
version, and enabled flag from `PluginSettings`. Enable and disable send
`settings.plugins.enable` and `settings.plugins.disable`. A row changes only
when `plugin_settings` carries a valid catalog and `show_effective` accepts it.

`plugin_settings_error` is shown with `error.plugins`. That event is not a
successful save: the page does not call `show_effective` for it, the previous
rows stay, and the same enable or disable can be sent again. A failed first
load does not present an empty catalog as installed. The page snapshot has no
saved or success flag.

The Electron session allowlist includes the three methods next to the other
settings methods. A rejected Desktop command is delivered as
`plugin_settings_error`, so the page keeps its rows. Package install, update,
and remove are not on this page.

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

Rust 1.99.1 for the page test. The workspace uses edition 2024. Wasm check used
nightly-2026-10-28 and gpui-kit `ec89cfc`.

Host evidence from `81e5850`, not re-run in this slice:

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

This slice:

- `cargo +stable test -p pero-ui --lib host_rejection` passed. A rejected
  event, including one that carries a different catalog, leaves the listed
  name, version, and enabled flag in place and records `error.plugins`.
  The same disable can be sent again. An invalid catalog does not replace
  the list. A later valid catalog does, and the notice is cleared. The
  snapshot has no saved flag.
- `node scripts/check/locales.mjs` checked 533 bilingual messages.
- `npx tsc --noEmit` passed.
- `PERO_GPUI_KIT_PATH` set to gpui-kit `ec89cfc`, then
  `node scripts/cargo.mjs +nightly-2026-10-28 check --locked -p pero-ui --target wasm32-unknown-unknown`
  passed. The settings view typechecked with the Wasm Desktop crate.

## Open follow-ups

- An Electron session that opens All settings, lists a seeded catalog, disables
  and enables a plugin, and shows `plugin_settings_error` while the previous
  rows stay and the control can be used again. That running-workbench evidence
  is what can check 「插件设置页」.
- Empty, loading, error, and disabled states in that session. The page draws
  them, and this record does not claim they were seen.
- 「插件管理」 still owes install, update, and remove. Until a package update
  writes a new version into this catalog, the page has no Host update to
  reflect. After that write, `settings.plugins.get` is the list the page shows.
