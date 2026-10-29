---
kind: verification_report
date: 2026-10-29
---

# 凭据设置页

ROADMAP item 「凭据设置页」 stays unchecked. Acceptance is: view whether a
credential is configured, replace and remove it, reopen to verify connection
status, and a failed save must not show success or echo the secret.

This note records the Host settings slice and the Desktop page. It does not
mark the item complete: the page has not been exercised in Electron.

## What landed

`settings.credentials.get` lists stored credential slots.
`settings.credentials` with `operation: replace` or `operation: remove` updates
one slot in the runtime settings file. The public `credential_settings` event
carries `id`, `label`, `configured: true` and connection `status: "configured"`.
The secret envelope stays in the private settings file under `credentials` and
is omitted from the event, from `Debug`, and from error text. A rejected
command returns `error.credentials` and leaves the file bytes unchanged. An
unreadable `credentials` section is not overwritten.

The Desktop settings page lists those slots and can replace or remove one.
The secret field is a password input. A rejected save shows `error.credentials`
and keeps the draft. A successful replace clears the secret from the draft and
from the field. Opening the page sends `settings.credentials.get` and shows
the saved connection status. A refresh that is still in flight cannot be
reported as a successful replace. The public diagnostic snapshot records
`secretPresent` and never the envelope.

Renderer session IPC forwards `settings.credentials` and
`settings.credentials.get`. The main process rewrites `credential_settings`
before delivery so a secret field or secret text cannot reach the page. The
forwarded command is not logged. A rejected renderer save becomes
`error.credentials` and does not clear the draft. Browser preview does not
pretend the save succeeded.

L0 coverage is the credential store table in `crates/pero-contracts`, the
settings-file test in `crates/pero-runtime`, the page transition test in
`crates/pero-ui`, the Host contract cases in `tests/contracts/host.mjs`
(happy path, validation, restart persistence, removal persistence), and
`desktop/src/main/session-command.test.ts`.

## Open follow-ups

- Electron evidence for the page: list, replace, remove, reopen, and a failed
  save that keeps the draft and does not show the secret. The ROADMAP checkbox
  stays open until that run exists.
- `status: "configured"` means an envelope is stored. There is no live
  provider handshake.
- OS credential storage remains the separate macOS, Windows, Linux, Android
  and iOS ROADMAP items. This slice uses the private settings file, same
  boundary as the encrypted remote-connection envelope.
