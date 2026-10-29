---
kind: verification_report
date: 2026-10-29
---

# App enable and disable

ROADMAP 「本地 App 平台」 already covers Host install, enable/disable, and the
Python backend, and that checkbox stays checked. This note records the Host
stdio contract for `disable` and for the catalog those lifecycle calls return.
It does not check 「App 目录」, 「App 卸载」, or 「App 恢复」.

## Passed

`enable_disable_refreshes_the_catalog_and_survives_restart` drives
`AppStore`: install, enable, and disable each return the installed catalog
with the new `enabled` flag. A repeated enable or disable keeps that flag.
Disable leaves the App listed, refuses `resolve`, and is still disabled after
`AppStore::open`. An unknown name returns "App is not installed" and does not
change the catalog.

`node tests/contracts/apps.mjs` runs the same sequence on a real `pero-host`
stdio process, including generated `AppRequest` / `AppsResultEvent` checks for
`action: "disable"`. Disable survives a Host restart. Re-enable, a second
restart, private `resolve`, and uninstall still pass. The public catalog in
those results has no `root` or backend token; `resolve` is the call that
returns the private access descriptor.

## Still open

Desktop frame disposal and the disabled state after a Desktop restart remain
the existing Electron evidence in [apps.md](apps.md). This slice does not add
a new Desktop run.

「App 目录」 still needs domain browsing, visible permission and environment
requirements, and a user-controlled order. Packaged-release acceptance is
unchanged.
