---
kind: verification_report
date: 2026-10-29
status: verified_local
---

# App directory requirements

ROADMAP 「App 目录」 stays unchecked. This slice publishes permission and
environment requirements on the Host catalog. It does not browse by domain,
persist a user-controlled order, or render the new fields in Desktop.

## Catalog fields

`apps` list, install, enable, disable, and uninstall replies include
`permissions` and `environment` on each record.

- `environment.runtime` is `none` when the App has no backend, and `python` or
  `node` when that is the backend. An explicit manifest `environment.runtime`
  that disagrees is refused at parse, which is the install boundary.
- `permissions.host` is the declared host permission list. This host still
  refuses a non-empty list, so every installed record publishes `[]`.
  Publishing the field does not grant a host capability.

Opening an installed App is unchanged and already covered by the App platform
checks. Adjusting order is not.

## Checked

On Rust stable 1.99.1:

- `cargo test -p pero-runtime apps --lib` — six App tests passed. The manifest
  table still rejects host permission grants. The catalog table covers omitted
  and matching runtimes and refuses mismatches. Install, enable, backend, and
  uninstall behavior is unchanged.
- `cargo build -p pero-host` and `node tests/contracts/apps.mjs` — real Host
  stdio install of a panel-only App returns `permissions.host: []` and
  `environment.runtime: none`. A package that declares `python` matching its
  backend is listed with that runtime and is not started. A `node` declaration
  on a Python backend is refused. Generated App result schemas match.

## Still open

- Browse and install by domain.
- A durable user order and a Desktop control to change it after install.
- Desktop does not show permission or environment requirements. The workbench
  catalog ignores the new fields.
- Unmet host permissions remain an install error, not a catalog row, because
  this host does not grant them.
- No packaged Desktop acceptance for this slice.
