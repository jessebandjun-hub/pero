---
kind: verification_report
date: 2026-10-29
status: verified_local
---

# App catalog order

ROADMAP 「App 目录」 stays unchecked. The Host saves a user-controlled catalog
order, and Desktop can send `reorder` from the Apps page. This slice does not
browse by domain, publish permission or environment requirements, or include a
packaged Electron session.

## Order

`apps` `reorder` takes `names`: each installed App once. The Host writes
`.catalog-order.json` in the managed App root only after that check passes, then
returns the catalog in that order. `list` and the other catalog replies use the
same sequence. A new Host process reads the file.

A request that repeats a name, names an App that is not installed, or omits an
installed App is rejected. The saved file and the installed packages stay as
they were. A name that is not an App name never reaches the store. An
unreadable order file does not remove Apps; listing falls back to name order
until a valid reorder replaces the file.

Until the user reorders, no order file is written and the list is name order.
After a saved order, names that are still installed keep their relative order.
Apps the file does not name, including ones installed later, follow in name
order. Uninstall does not rewrite the file; listing skips the removed name.

## Desktop

The Apps page shows Move up and Move down on each installed App. Those controls
are disabled while a request is in flight, at the end of that direction, and in
the native preview, same as the other App actions. A click sends `apps_reorder`
with every catalog name after that neighbor swap.

`DesktopApps.command` accepts `reorder` on the apps action allowlist. It writes
`{ method: "apps", action: "reorder", names }` to the Host. Success publishes
`apps_catalog` in the order the Host returned, which is what the page renders.
A value that is not a list of App names is refused in the main process and
shown as `apps_error`, and the Host is not asked. A permutation the Host
rejects is also shown as `apps_error`. The page leaves the previous catalog in
place because `apps_catalog` is not published.

`reorder` is not on the Electron session method allowlist. Session IPC still
forwards only the existing session actions and `settings.workbench` methods.
App requests stay on `pero:apps`, including `resolve`, so a session command
cannot ask the Host for a backend token by calling `reorder` or any other App
action.

## Checked

Host evidence below is the catalog-order slice already on this branch. It was
not re-run for the Desktop wire-up. Rust 1.99.1. `pero-runtime` and `pero-host`
were built with Cargo directly. `scripts/cargo.mjs` was not used: this
environment has no GPUI kit checkout, and these two crates do not link it.

- `cargo test -p pero-runtime --lib apps` — 8 passed. The order tables cover
  projection and rejected permutations. The store test saves an order, reopens
  it, rejects a bad reorder without changing the file, keeps that order across
  a later install and uninstall, and still lists the packages when the order
  file is not JSON.
- `cargo build -p pero-host` and `node tests/contracts/apps.mjs` — real Host
  stdio. Three panel Apps list in name order, `reorder` survives a new Host
  process, and duplicate, unknown, omitted, and malformed names are rejected
  with the catalog unchanged. Generated App request and result schemas match.
- `node scripts/codegen/contracts.mjs --check` — generated Host contracts were
  current.

Desktop, on this slice, without GPUI and without Electron:

- `node --experimental-transform-types --experimental-test-module-mocks --test desktop/src/main/apps-command.test.ts`
  — 3 passed. `apps_reorder` writes `method: "apps"`, `action: "reorder"`, and
  the names, then publishes `apps_catalog` from the Host result. `Nope` and a
  non-array are `apps_error` with no Host write. A duplicate list is forwarded
  and the Host error is shown without `apps_catalog`. `apps_resolve` stays off
  the renderer allowlist.
- `cargo +1.99.1 test -p pero-ui --lib apps` — 2 passed, including the neighbor
  swap the Move buttons send. The GPUI Apps page itself was not compiled: the
  view is behind the wasm/native cfg, and there is no GPUI kit checkout.

`Cargo.lock` was restored after that UI test. Cargo 1.99 wanted to record
registry GPUI crates that this checkout does not patch in.

## Still open

- Browse and install by domain.
- Permission and environment requirements. Those fields are not in this change.
- No packaged or running Electron session of Move up / Move down. 「App 目录」
  stays unchecked until that acceptance exists.
