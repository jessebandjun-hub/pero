---
kind: verification_report
date: 2026-10-29
---

# Automation settings

ROADMAP item 「自动化设置页」 stays unchecked. Host can view, edit, and create
automations in the settings file. The Desktop settings section renders that
catalog, edits a stored record, and can append a new one. This slice sits on
the Host create commit `94d4e18` (itself on catalog commit `c61d38a`, not only
on `master` `604cca1`). Evidence here is L0. A running Electron session has
not been recorded, so the checkbox stays open.

## Stored catalog

`AutomationCatalog::apply` is the only pause, resume, update, and create
transition. Each success is the catalog written to the Host settings file. A
rejected edit returns no next catalog and does not write. Reading a missing
`automations` value yields an empty catalog and does not create the file.

`settings.automation.create` appends one unpaused automation. The caller
supplies the id, name, and instruction. A blank name, a blank instruction, a
duplicate id, or a catalog that would exceed 64 records is rejected and the
file bytes stay as they were.

`AutomationEditor` is the page state for an existing record. `shown_paused`
comes from the saved catalog. `shown_text` stays on the typed draft until
`show_effective` receives a stored catalog containing that same name and
instruction. An `automation_settings_error` carries no catalog, so the page
must not call `show_effective` for it. The editor does not start a new record.

Host methods:

- `settings.automation.get`
- `settings.automation.pause`
- `settings.automation.resume`
- `settings.automation.update`
- `settings.automation.create`

Success event: `automation_settings`, `data` is the stored catalog.
Failure event: `automation_settings_error` with `error.automation`.

## Desktop page

All settings renders the catalog through `AutomationPage`, which folds
`automation_settings.data` with `AutomationEditor`. The section shows empty,
loading, error, and disconnected states. Pause, resume, and save send
`settings.automation.pause`, `resume`, and `update`. Electron forwards those
methods to the Host. `automation_settings_error` does not replace the typed
draft or the input fields. A rejected save still shows the stored catalog in
`data`, including the stored name next to the draft.

An empty catalog opens a create form for id, name, and instruction. `New
automation` opens the same form beside stored records. Create sends
`settings.automation.create`. The Electron session allowlist includes that
method. A rejected create keeps the typed id, name, and instruction and leaves
the stored catalog on screen. A successful `automation_settings` event replaces
the shown catalog and selects the created record. The editor still does not
invent the stored record; the Host catalog in that event is the refresh.

## Verification

- `cargo test -p pero-contracts -p pero-runtime --lib automation` passed:
  pause, edit, resume, and create return the stored catalog; a blank name or
  instruction, a duplicate id, and a 65th record leave the catalog unchanged.
- `node tests/contracts/host.mjs` passed against `pero-host --stdio`. A blank
  name and a blank instruction on an empty catalog emitted
  `automation_settings_error` and left the settings file bytes unchanged.
  Create then matched the file. Pause, update, and resume of a stored record
  still matched the file. A later blank name, blank instruction, and duplicate
  id left that file unchanged, and a second create was still present after
  Host restart. Locale and workbench preferences remained.
- `cargo +nightly-2026-10-28 test -p pero-ui --lib automation_page` passed:
  empty, loading, error, and disconnected screens; pause, resume, and update;
  draft retention when `automation_settings_error` or an invalid catalog
  arrives; a rejected create keeps the typed id, name, and instruction; a
  successful catalog event selects the new record and shows that stored
  catalog.
- `cargo +nightly-2026-10-28 check -p pero-ui --target wasm32-unknown-unknown`
  type-checked the settings section. It stopped on the existing
  `include_bytes!` for `web/public/fonts/*.ttf`, which are generated and not
  in this checkout. No Electron session was run.

## Open follow-ups

- No Electron recording of the settings section: empty, loading, error,
  disconnected, pause, resume, a rejected save that keeps the typed draft, or
  create. That includes a rejected create keeping the typed id, name, and
  instruction, and a successful create showing the refreshed catalog.
- The ROADMAP checkbox stays open until that Desktop session is recorded.
