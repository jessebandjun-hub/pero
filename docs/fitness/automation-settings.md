---
kind: verification_report
date: 2026-10-29
---

# Automation settings

ROADMAP item 「自动化设置页」 stays unchecked. Host catalog commands and the
Desktop settings section are in place. Evidence is L0 on top of `master`
`604cca1`. A running Electron session has not been recorded, so the checkbox
stays open.

## Stored catalog

`AutomationCatalog::apply` is the only pause, resume, and update transition.
Each success is the catalog written to the Host settings file. A rejected
edit returns no next catalog and does not write. Reading a missing
`automations` value yields an empty catalog and does not create the file.

There is no create command. The page edits records that are already stored.

`AutomationEditor` is the page state. `shown_paused` comes from the saved
catalog. `shown_text` stays on the typed draft until `show_effective` receives
a stored catalog containing that same name and instruction. An
`automation_settings_error` carries no catalog, so the page must not call
`show_effective` for it.

Host methods:

- `settings.automation.get`
- `settings.automation.pause`
- `settings.automation.resume`
- `settings.automation.update`

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

Creating an automation is still out of scope. The page only edits records the
Host already stored.

## Verification

- `cargo test -p pero-contracts -p pero-runtime --lib automation` passed:
  pause, edit, and resume return the stored catalog; rejected edits leave it
  unchanged; a failed save keeps the draft while a later pause still shows the
  stored paused flag.
- `node tests/contracts/host.mjs` passed against `pero-host --stdio`. Get,
  pause, update, and resume each matched the file. A blank name and an unknown
  id emitted `automation_settings_error` and left the stored catalog in place.
  Restart returned the same catalog. Locale and workbench preferences remained.
- `cargo test -p pero-ui --lib automation_page` passed: empty, loading, error,
  and disconnected screens, the pause / resume / update commands, and draft
  retention when `automation_settings_error` or an invalid catalog arrives.
  `cargo check -p pero-ui --target wasm32-unknown-unknown` compiled the settings
  section. No Electron session was run.

## Open follow-ups

- No Electron recording of the settings section: empty, loading, error,
  disconnected, pause, resume, and a rejected save that keeps the typed draft.
- Creating an automation is outside this item.
