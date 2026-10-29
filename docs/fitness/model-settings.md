---
kind: verification_report
date: 2026-10-29
---

# 模型设置页 acceptance

ROADMAP item 「模型设置页」 stays unchecked. The item text is: 编辑 Provider、模型与默认预设，复用现有 Host 配置；验收：重开显示生效值，认证失败可返回修改且不丢草稿。

This note records the Host slice. It does not mark the item complete.

## Host settings file

`settings.model.get` and `settings.model` read and write `model_settings` in the existing runtime settings file (`PERO_SETTINGS_PATH`), beside locale, workbench, and remote connection. The value is Provider, model, and default preset.

- A saved value is the effective value returned by get.
- An empty file returns `model_settings` with no `data`.
- Invalid values, including surrounding whitespace, empty fields, unknown fields, and over-long fields, return `error.preferences` and leave the file bytes unchanged.
- A new Host process returns the same effective value.

L0 evidence is the domain table in `crates/pero-contracts/src/model_settings.rs`, the settings-file reload in `crates/pero-runtime/src/settings.rs`, and `tests/contracts/host.mjs` (happy path, validation, and Host restart).

## Still open

- Desktop page that edits Provider, model, and default preset.
- Reopening that page shows the effective value.
- Authentication failure returns to the editor and keeps the draft. Save does not authenticate a provider, and the settings file stores only the effective value.
- The saved value is not applied to a new ACP session, and it is not the harness `agent-default-model` section. Those belong to 「模型与预设」.
