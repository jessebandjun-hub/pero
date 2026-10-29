---
kind: verification_report
date: 2026-10-29
---

# Automation settings

ROADMAP item 「自动化设置页」 stays unchecked. Host can view, edit, and create
automations in the settings file. This create slice sits on the Host catalog
commit `c61d38a` (not on `master` `604cca1`). It is L0 evidence. It does not
mark the item complete.

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

## Open follow-ups

- Desktop create control. The sibling settings page
  (`cursor/desktop-automation-settings-822d`) loads, pauses, edits, and resumes
  records that already exist. It does not send `settings.automation.create`,
  and the Electron session command allowlist does not include that method.
- No Electron evidence that a created automation shows up in the running
  workbench, including empty, loading, error, and disabled states for that
  action.
- On `automation_settings_error` during create, the running page must keep the
  typed name and instruction. That behavior is not covered here.
