---
kind: verification_report
date: 2026-10-29
---

# 渠道设置页

ROADMAP item 「渠道设置页」 stays unchecked. Acceptance is: edit channel
connection parameters, view connection status, and after a bad credential
show a recovery entry so a corrected credential can connect and disconnect.

This note records the Host slice. It does not mark the item complete.

## What landed

Channel parameters live in the runtime settings file (`PERO_SETTINGS_PATH`)
under `channels`, beside locale, workbench, and remote connection. There is
no second store.

- `settings.channel.get` returns the saved catalog.
- `settings.channel.set` stores kind, endpoint, account, and an accepted
  credential envelope. The public `channel_settings` event reports
  `credential_configured: true` and does not include the envelope.
- `settings.channel.connect` stores `connected` when the saved credential is
  accepted.
- `settings.channel.disconnect` stores `disconnected` and keeps the parameters.
- Kinds are the seven channels named by 「渠道连接」: WeChat, WeCom, Feishu,
  DingTalk, Slack, Teams, and Discord. This slice does not open those networks.
- A credential envelope is rejected when it is not a padded base64 envelope,
  or when the decoded bytes start with `reject:`. `settings.channel.set` then
  returns `channel_settings_error` with `status: "error"`,
  `reason: "credential_rejected"`, and `recovery: "replace_credential"`.
  The settings file bytes stay as they were, including a channel that is
  already connected.
- `settings.channel.connect` on a stored rejected credential keeps the
  endpoint, account, and credential, and records `status: "error"` with the
  same reason and recovery. It does not report connected. A later get,
  including after a Host restart, still returns that recovery.
- Invalid parameters and an unknown channel return `recovery: "edit_parameters"`
  and do not rewrite the file. An unreadable `channels` section is not
  overwritten.
- Connect, disconnect, and the recorded error survive a new Host process.

L0 coverage is the catalog table in `crates/pero-contracts/src/channel.rs`,
the settings-file reload in `crates/pero-runtime/src/settings.rs`, and
`tests/contracts/host.mjs` (rejection, connect, disconnect, restart).

## Still open

- Desktop 「渠道设置页」 that edits parameters and shows connected,
  disconnected, and error.
- A recovery control bound to `recovery: "replace_credential"`. The Host
  returns the code; it does not draw the control. After the credential is
  replaced, the page must call connect, then disconnect.
- Renderer session IPC still does not forward `settings.channel.*`
  (`desktop/src/main/index.ts`). The main process would own the envelope,
  matching `settings.remote`.
- No call to WeChat, WeCom, Feishu, DingTalk, Slack, Teams, or Discord.
  `reject:` is a local stand-in so a bad credential has a deterministic
  recovery path. 「渠道连接」 remains open.
- Connector catalog, OAuth, and account permission UI are out of scope.
- The envelope is in the private settings file, not the OS credential store.
- ROADMAP 「渠道设置页」 stays unchecked until Desktop evidence exists.
