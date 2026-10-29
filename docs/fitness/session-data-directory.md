---
kind: verification_report
date: 2026-10-29
status: verified_local
---

# 会话数据目录

ROADMAP item 「会话数据目录」 stays unchecked. This slice migrates the
development journal into the user-data journal on Host start. It does not add
session search, rename, pin, archive, or resume UX.

## Behavior

The development journal is `{Host working directory}/target/sessions.jsonl`.
That is the file the Host used when `PERO_HISTORY_PATH` was unset. The live
journal is chosen in this order:

1. `PERO_HISTORY_PATH`
2. `sessions.jsonl` beside `PERO_SETTINGS_PATH`
3. `sessions.jsonl` under the platform user data directory (`$XDG_DATA_HOME/pero`,
   `~/Library/Application Support/pero`, or `%APPDATA%/pero`)

Startup copies development records that are not already in the live journal.
The development file is not deleted or rewritten. After a successful copy, new
records append only to the user-data file, and a later start reads that file.
If the copy cannot be written, the Host keeps reading whichever of the two
journals is already a readable file, emits `error.session_history`, and leaves
both copies on disk.

Desktop development launches the Host with the repository as its working
directory, so this is the `target/sessions.jsonl` beside the checkout. A
packaged Desktop launch uses the home directory as the Host working directory
and does not scan the repository.

## L0 evidence

`scripts/cargo.mjs` exits until `PERO_GPUI_KIT_PATH` points at a gpui-kit
checkout. `pero-host` does not link that crate, so these commands used
`cargo +stable` (rustc 1.99.1) instead. `Cargo.lock` was left unchanged.

- `cargo +stable test -p pero-host --bin pero-host` — 6 passed, including the
  five session-journal cases and the existing workspace-origin case.
- `cargo +stable build -p pero-host`
- `node tests/contracts/host.mjs` — `Host protocol conformance passed.`
  Observed 32/47 declared event kinds. The new scenario is in that script.

The Rust tests require the merged journal to keep every record, the user-data
file to match the development session count and record text, a second open to
read that user-data file, and an appended record to survive another open
without being written back to `target/sessions.jsonl`. They also require a
blocked destination and a destination that is a directory to leave the
development bytes unchanged, and an interrupted `sessions.jsonl.previous`
backup to become the user-data journal when that file is missing.

The Host contract starts `pero-host --stdio` with a prepared
`target/sessions.jsonl`, checks the user-data journal and the `replay` event
for the same two sessions and the same records, appends one record, and checks
a second process. It then points `PERO_HISTORY_PATH` through a parent that is a
file and checks that both processes still replay the development journal, emit
`error.session_history`, and leave that file unchanged.

`node scripts/check/locales.mjs` checked 524 bilingual messages, including
`error.session_history`.

## Gaps

- No Electron launch was run against a repository `target/sessions.jsonl` and
  the Desktop `userData` profile. The contract drives the Host process directly.
- Packaged Desktop does not migrate a checkout's `target/sessions.jsonl`,
  because its Host working directory is the home directory.
- The failure cases make the destination path unusable. They do not simulate a
  read-only directory; this environment can still write as root, so a mode bit
  would not block the copy.
- 「会话管理」 and 「会话续接」 are unchanged. Replay after migration shows the
  saved records; it does not resume an Agent turn.

Do not check the ROADMAP item until a Desktop restart against an existing
development journal is recorded.
