---
kind: verification_report
date: 2026-10-29
---

# 模型设置页 acceptance

ROADMAP item 「模型设置页」 stays unchecked. The item text is: 编辑 Provider、模型与默认预设，复用现有 Host 配置；验收：重开显示生效值，认证失败可返回修改且不丢草稿。

Host persistence and the Desktop settings section are in place. Evidence is L0. A running Electron session has not been recorded, so the checkbox stays open.

## Host settings file

`settings.model.get` and `settings.model` read and write `model_settings` in the existing runtime settings file (`PERO_SETTINGS_PATH`), beside locale, workbench, and remote connection. The value is Provider, model, and default preset.

- A saved value is the effective value returned by get.
- An empty file returns `model_settings` with no `data`.
- Invalid values, including surrounding whitespace, empty fields, unknown fields, and over-long fields, return `error.preferences` and leave the file bytes unchanged.
- A new Host process returns the same effective value.

L0 evidence is the domain table in `crates/pero-contracts/src/model_settings.rs`, the settings-file reload in `crates/pero-runtime/src/settings.rs`, and `tests/contracts/host.mjs` (happy path, validation, and Host restart).

Save does not authenticate a provider. The settings file stores only the effective value. There is no provider round-trip on `settings.model`, so an authentication failure is not a distinct Host result.

## Desktop page

All settings renders Provider, model, and default preset through `ModelPage`. Electron forwards `settings.model.get` and `settings.model`. The section shows empty, loading, error, and disconnected states.

Opening the page requests `settings.model.get` and fills the form with the effective value, including an empty form when nothing is saved. A later get that was started as a refresh keeps an uncommitted draft.

`error.preferences` (rejected save) and `error.connection` (send failure or disconnect while a get or save is in flight) leave the typed draft in the fields. The saved line under the form stays on the previous effective value. The editor stays on screen; there is no separate authentication step to return from.

The saved value is not applied to a new ACP session.

## Verification

- `cargo test -p pero-contracts -p pero-runtime --lib model_settings` covers the Host value rules.
- `tests/contracts/host.mjs` covers get, save, rejection, an unchanged file, and Host restart.
- `cargo test -p pero-ui --lib model_page` covers empty, loading, the effective value on reopen, and draft retention for `error.preferences`, `error.connection`, and disconnect during save.
- `cargo check -p pero-ui --target wasm32-unknown-unknown` typechecks the settings section against gpui-kit 0.8.1. It then stops on the generated font files, which are not in git. No Electron session was run.

## Still open

- No Electron recording of the settings section: empty, loading, a reopen that shows the effective value, and a rejected save that keeps the typed draft.
- Applying the saved value to a new ACP session belongs to 「模型与预设」.
