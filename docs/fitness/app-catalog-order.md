---
kind: verification_report
date: 2026-10-29
status: verified_local
---

# App catalog order

ROADMAP 「App 目录」 stays unchecked. This slice saves a user-controlled catalog
order in the Host. It does not browse by domain, publish permission or
environment requirements, or add a Desktop control.

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

## Checked

Rust 1.99.1. `pero-runtime` and `pero-host` were built with Cargo directly.
`scripts/cargo.mjs` was not used: this environment has no GPUI kit checkout,
and these two crates do not link it.

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

## Still open

- Browse and install by domain.
- Permission and environment requirements. Those fields are not in this change.
- Desktop has no reorder control. `DesktopApps.command` accepts list, install,
  enable, disable, uninstall, and open. The workbench renders `apps_catalog`
  in the order the Host returned, and it does not send `reorder`.
- No packaged Desktop acceptance for this slice.
