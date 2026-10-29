---
kind: verification_report
date: 2026-10-29
---

# Adjustable layout

ROADMAP item 「可调布局」 stays unchecked. Acceptance is a draggable sidebar and
panel, a navigation drawer on a narrow window that can still switch the main
column, and restoration of dragged widths. This note records the L0 width model.
It does not mark the item complete.

## What this slice covers

`WorkbenchLayout` in `crates/pero-contracts` owns the sidebar and panel widths.
An unmodified layout still presents the historical docked sidebar: 240 logical
pixels below 1150, otherwise 260. Viewports below 1000 present a drawer. While
that drawer is closed the main column keeps the full width, so a navigation
choice still switches the main content. Opening the drawer reserves the sidebar;
choosing a section closes it again.

A drag clamps each column (sidebar 200–420, panel 240–560). The first drag
snapshots the width that was actually on screen, so a panel drag at 1000 pixels
does not jump the compact sidebar from 240 to 260. Double-clicking either
handle restores the automatic widths. Host workbench preferences persist a
custom layout and omit it until the user drags, so older preference files still
load. The Desktop window minimum is 720 so a window can enter the drawer; at
1000 and wider the sidebar stays docked, which is what the existing narrow
fixtures measure.

## Passed

- `WorkbenchLayout` unit tests: docked widths, drawer open/closed reserved
  width, clamp, non-finite drag, restore, and JSON round-trip.
- Workbench preference save/load keeps a custom layout beside appearance and
  project bindings, rejects an out-of-range width without replacing the file,
  and omits an unmodified layout.

## Open follow-ups

- No Desktop e2e drives a pointer drag, a double-click restore, or a window
  below 1000 pixels. `npm run test:e2e` still only screenshots the docked
  1000-pixel window.
- The panel handle is on the contribution list, the existing fixed secondary
  column. Chat, projects, and other sections do not yet have their own split.
- Keyboard resizing is out of scope for this slice.
