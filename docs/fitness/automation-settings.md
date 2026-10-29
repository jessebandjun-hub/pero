---
kind: verification_report
date: 2026-10-29
---

# Automation settings

ROADMAP item 「自动化设置页」 stays unchecked. This slice is the Host catalog
the Desktop page will view and edit. It is L0 evidence on top of `master`
`604cca1`. It does not mark the item complete.

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

## Verification

- `cargo test -p pero-contracts -p pero-runtime --lib automation` passed:
  pause, edit, and resume return the stored catalog; rejected edits leave it
  unchanged; a failed save keeps the draft while a later pause still shows the
  stored paused flag.
- `node tests/contracts/host.mjs` passed against `pero-host --stdio`. Get,
  pause, update, and resume each matched the file. A blank name and an unknown
  id emitted `automation_settings_error` and left the stored catalog in place.
  Restart returned the same catalog. Locale and workbench preferences remained.

## Open follow-ups

- Desktop settings page that loads the catalog, sends pause, update, and
  resume, and renders `automation_settings.data` through `AutomationEditor`.
- On `automation_settings_error`, leave the draft on screen.
- Empty, loading, error, and disabled states in the running workbench.
- Creating an automation is outside this item.
