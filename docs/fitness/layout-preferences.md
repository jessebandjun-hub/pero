---
kind: verification_report
date: 2026-10-29
---

# Workbench layout and theme preferences

Theme (light, dark, and system) and project bindings already persist in the Host
runtime settings file. This slice adds the sidebar width to that same
`workbench` object.

## What this proves

- A sidebar width from 200 to 420 CSS pixels round-trips through
  `settings.workbench`, survives a Host restart, and is returned by
  `settings.workbench.get`.
- Widths outside that range are rejected and leave the saved preferences
  unchanged. A missing `layout` loads as the default 260px sidebar.
- Locale and remote-connection writes leave the saved width in place.
- The workbench paints the saved width when the window is at least 1150px wide.
  Narrower windows keep the existing 240px sidebar so the main column stays usable.
  Dragging the sidebar edge sends the new width through the same Host command.
  The compact width is not written back.
- While appearance is `system`, an OS theme change still updates the painted
  theme. That path was already in the workbench and is unchanged here.

## How to verify

```sh
cargo test -p pero-contracts -p pero-runtime layout -- --nocapture
cargo test -p pero-contracts -p pero-runtime preferences_survive
npm run test:contracts
```

Desktop confirmation still required before checking the ROADMAP item:

- In Electron, drag the sidebar, quit, and reopen. The wide window should
  restore that width; a window under 1150px should still use the compact sidebar.
- Switch appearance to light, dark, and system, restart, and change the OS theme
  while system is selected. `npm run test:sidebar` already covers appearance
  restart and live system theme; repeat it on the target machine with this build.
- Keyboard resize and the narrow-window navigation drawer belong to 可调布局
  and are not part of this slice.

The ROADMAP checkbox stays open until that Desktop restart is recorded.
