---
kind: verification_report
date: 2026-10-29
---

# App 恢复

ROADMAP item 「App 恢复」 stays unchecked. Acceptance is: reopen an App that
failed to start or exited abnormally, against its original domain data, and
keep a locatable error record when recovery fails. This note records L0
evidence on top of `master` `604cca1`. It does not mark the item complete.

## What this slice covers

`AppStore::resolve` remains the Host open path (`action: resolve`). No new
stdio action was added. An enabled App whose backend is missing or has exited
is started again with the installed package and the existing
`app-data/<name>` directory. Domain files are not replaced. Disable, uninstall
and install keep their current behavior; uninstall still leaves `app-data` in
place, including a recovery record if the last open failed.

A failed restart is written to `app-data/<name>/.pero/recovery-error.json`.
The resolve error includes that path. The file names the App, the data
directory and the start error. A later successful open removes the record.

Library coverage:

- `abnormal_exit_reopens_original_app_data` — a Python backend that exits is
  opened again; the same `draft.json` is served and no recovery record remains.
- `failed_start_keeps_original_data_and_recovery_record` — a backend that
  exits before readiness leaves `draft.json` and a parseable recovery record.
  Replacing the installed entry and opening again serves the App and deletes
  the record.

## Open follow-ups

- Desktop was not run. Electron already forwards a failed `resolve` as
  `apps_error`, so the record path would be in that text, but there is no
  browser evidence for a failed start, an abnormal exit or the retained data.
- The public catalog has no recovery field. The locator is the resolve error
  and `app-data/<name>/.pero/recovery-error.json`.
- No packaged Desktop run and no Virtual Cell crash drill.

Verified with `cargo test -p pero-runtime --lib apps::`: 7 passed, including
the two recovery tests and the existing install, seed, refusal and backend
lifecycle tests. With the repository GPUI patch configured, the same filter is:

```sh
node scripts/cargo.mjs test -p pero-runtime --lib apps::
```
