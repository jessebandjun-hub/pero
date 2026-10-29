---
kind: verification_report
date: 2026-10-29
---

# App 卸载 acceptance

ROADMAP item 「App 卸载」 stays unchecked. Acceptance is: stop and uninstall an
installed App; the navigation entry and background process disappear; on
failure, show what actually remains. This note records L0 evidence. It does
not mark the item complete.

## Host uninstall

`cargo test -p pero-runtime --lib apps::` passed 8 App tests on 2026-10-29
(Rust 1.99.1, direct Cargo, no GPUI patch). Three of them cover this item:

- `uninstall_stops_backend_and_removes_the_app` enables an App, talks to its
  live Python backend, then uninstalls. The catalog is empty, the package
  directory is gone, domain data under `app-data/` remains, the backend port
  refuses connections, and a reopened store still lists nothing.
- `failed_uninstall_reports_the_installation_that_remains` (Unix) makes the
  package directory non-writable while the backend is running. Removal fails.
  The error reports that the package is still installed, the backend is not
  running, and `virtual-cell` is still listed. `list()` matches that catalog,
  `app.json` is still present, the old port refuses connections, and resolving
  the same App starts a new backend.
- `uninstall_failure_drops_a_missing_package_from_the_catalog` deletes the
  package directory first. The error reports that the package is no longer on
  disk, the backend is not running, and nothing is still listed.

Host `apps_result` failures stay `{ok:false, error}`. The error string is that
retained-state sentence. `pero-host` maps `UninstallError` through `Display`.

Desktop, after an uninstall failure, requests `list` and sends `apps_catalog`
before `apps_error`. The Apps library then shows the catalog the Host still
tracks, and the banner shows the sentence. `replace_catalog` already clears
the open panel when that catalog no longer contains it. The Web shell drops a
frame when the catalog no longer lists that App as enabled.

## Open follow-ups

- No Desktop run shows the Apps library row and the Web frame disappearing
  after a successful uninstall.
- No Desktop run shows the failure banner next to the row that remains.
- The sidebar Apps control is the library section. It stays after the last
  App is removed; the per-App entry is the library row.
- A failed uninstall that leaves the App enabled does not drop a previously
  opened frame. That frame was not re-checked.
- A partial delete that leaves a directory without `app.json` is reported as
  incomplete and still listed. That shape was not forced in a test.
- The sentence is English, as with the other Host App errors.
