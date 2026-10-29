---
kind: verification_report
date: 2026-10-29
---

# 凭据设置页

ROADMAP item 「凭据设置页」 stays unchecked. Acceptance is: view whether a
credential is configured, replace and remove it, reopen to verify connection
status, and a failed save must not show success or echo the secret.

This note records the Host settings slice. It does not mark the item complete.

## What landed

`settings.credentials.get` lists stored credential slots.
`settings.credentials` with `operation: replace` or `operation: remove` updates
one slot in the runtime settings file. The public `credential_settings` event
carries `id`, `label`, `configured: true` and connection `status: "configured"`.
The secret envelope stays in the private settings file under `credentials` and
is omitted from the event, from `Debug`, and from error text. A rejected
command returns `error.credentials` and leaves the file bytes unchanged. An
unreadable `credentials` section is not overwritten.

L0 coverage is the credential store table in `crates/pero-contracts`, the
settings-file test in `crates/pero-runtime`, and the Host contract cases in
`tests/contracts/host.mjs` (happy path, validation, restart persistence,
removal persistence).

## Open follow-ups

- Desktop 「凭据设置页」 that lists slots and can replace or remove one.
- Reopen that page and show the saved connection status.
- Failed-save UI that stays on the entered values and does not show success.
- Renderer session IPC still rejects `settings.credentials`, matching
  `settings.remote`: the main process would own the envelope.
- `status: "configured"` means an envelope is stored. There is no live
  provider handshake.
- OS credential storage remains the separate macOS, Windows, Linux, Android
  and iOS ROADMAP items. This slice uses the private settings file, same
  boundary as the encrypted remote-connection envelope.
