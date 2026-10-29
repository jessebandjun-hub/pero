//! Docked workbench column widths and the narrow-window navigation drawer.
//!
//! Widths are logical pixels. The shell passes the viewport in; this module
//! does not read a window. A layout that has never been dragged keeps the
//! historical docked sidebar: 240 below 1150, otherwise 260. Viewports below
//! 1000 dock navigation in a drawer so the main column stays reachable. The
//! Desktop window minimum used to be 1000, which is why existing narrow
//! fixtures stay docked.

use serde::{Deserialize, Serialize};

/// Which column a drag adjusts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutColumn {
    Sidebar,
    Panel,
}

impl LayoutColumn {
    pub const fn bounds(self) -> (u32, u32) {
        match self {
            Self::Sidebar => (WorkbenchLayout::SIDEBAR_MIN, WorkbenchLayout::SIDEBAR_MAX),
            Self::Panel => (WorkbenchLayout::PANEL_MIN, WorkbenchLayout::PANEL_MAX),
        }
    }
}

/// Saved column widths. `custom` is false until a drag changes a width.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkbenchLayout {
    pub sidebar: u32,
    pub panel: u32,
    pub custom: bool,
}

impl Default for WorkbenchLayout {
    fn default() -> Self {
        Self {
            sidebar: Self::SIDEBAR_DEFAULT,
            panel: Self::PANEL_DEFAULT,
            custom: false,
        }
    }
}

impl WorkbenchLayout {
    pub const SIDEBAR_DEFAULT: u32 = 260;
    pub const SIDEBAR_COMPACT: u32 = 240;
    pub const SIDEBAR_MIN: u32 = 200;
    pub const SIDEBAR_MAX: u32 = 420;
    pub const PANEL_DEFAULT: u32 = 320;
    pub const PANEL_MIN: u32 = 240;
    pub const PANEL_MAX: u32 = 560;
    /// Dock the sidebar at this width and above. Below it, navigation is a drawer.
    pub const DOCK_AT: f32 = 1000.;
    /// Uncustomized docked sidebars use [`Self::SIDEBAR_COMPACT`] below this width.
    pub const COMPACT_BELOW: f32 = 1150.;

    pub fn is_unmodified(layout: &Self) -> bool {
        layout == &Self::default()
    }

    pub fn valid(self) -> bool {
        let (sidebar_min, sidebar_max) = LayoutColumn::Sidebar.bounds();
        let (panel_min, panel_max) = LayoutColumn::Panel.bounds();
        if !self.custom {
            return self.sidebar == Self::SIDEBAR_DEFAULT && self.panel == Self::PANEL_DEFAULT;
        }
        (sidebar_min..=sidebar_max).contains(&self.sidebar)
            && (panel_min..=panel_max).contains(&self.panel)
    }

    /// Clear a drag and return to the historical automatic widths.
    pub fn restore(self) -> Self {
        Self::default()
    }

    /// Apply a horizontal drag. `displayed` is the width on screen at mouse-down.
    /// A delta that rounds back to that width leaves an unmodified layout unchanged.
    pub fn drag(self, viewport: f32, column: LayoutColumn, displayed: u32, delta_px: f32) -> Self {
        if !delta_px.is_finite() {
            return self;
        }
        let width = (displayed as f32 + delta_px).round();
        if !width.is_finite() {
            return self;
        }
        let (min, max) = column.bounds();
        let width = width.clamp(min as f32, max as f32) as u32;
        if width == displayed {
            return self;
        }
        let mut next = if self.custom {
            self
        } else {
            let chrome = self.present(viewport, false);
            Self {
                sidebar: chrome.sidebar,
                panel: chrome.panel,
                custom: true,
            }
        };
        match column {
            LayoutColumn::Sidebar => next.sidebar = width,
            LayoutColumn::Panel => next.panel = width,
        }
        next.custom = true;
        next
    }

    pub fn present(self, viewport: f32, drawer_open: bool) -> LayoutChrome {
        let sidebar = if self.custom {
            clamp_u32(self.sidebar, LayoutColumn::Sidebar.bounds())
        } else if viewport.is_finite() && viewport < Self::COMPACT_BELOW {
            Self::SIDEBAR_COMPACT
        } else {
            Self::SIDEBAR_DEFAULT
        };
        let panel = if self.custom {
            clamp_u32(self.panel, LayoutColumn::Panel.bounds())
        } else {
            Self::PANEL_DEFAULT
        };
        let drawer = viewport.is_finite() && viewport < Self::DOCK_AT;
        let open = drawer && drawer_open;
        LayoutChrome {
            mode: if drawer {
                LayoutMode::Drawer
            } else {
                LayoutMode::Docked
            },
            drawer_open: open,
            sidebar,
            panel,
            reserved_left: if open || !drawer { sidebar } else { 0 },
        }
    }
}

fn clamp_u32(value: u32, bounds: (u32, u32)) -> u32 {
    value.clamp(bounds.0, bounds.1)
}

/// How the shell should place the sidebar for one viewport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutChrome {
    pub mode: LayoutMode,
    pub drawer_open: bool,
    pub sidebar: u32,
    pub panel: u32,
    /// Width the main column yields on the left. Zero while the drawer is closed.
    pub reserved_left: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    Docked,
    Drawer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_clamp_restore_and_narrow_drawer_keeps_the_main_column() {
        let layout = WorkbenchLayout::default();
        for (viewport, open, mode, reserved, sidebar) in [
            (1488., false, LayoutMode::Docked, 260, 260),
            (1150., false, LayoutMode::Docked, 260, 260),
            (1149., false, LayoutMode::Docked, 240, 240),
            (1000., false, LayoutMode::Docked, 240, 240),
            (999., false, LayoutMode::Drawer, 0, 240),
            (999., true, LayoutMode::Drawer, 240, 240),
            (720., false, LayoutMode::Drawer, 0, 240),
            (720., true, LayoutMode::Drawer, 240, 240),
            (f32::NAN, true, LayoutMode::Docked, 260, 260),
        ] {
            let chrome = layout.present(viewport, open);
            assert_eq!(chrome.mode, mode, "mode {viewport}");
            assert_eq!(chrome.reserved_left, reserved, "reserved {viewport} {open}");
            assert_eq!(chrome.sidebar, sidebar, "sidebar {viewport}");
            assert_eq!(chrome.drawer_open, open && mode == LayoutMode::Drawer);
            assert_eq!(chrome.panel, WorkbenchLayout::PANEL_DEFAULT);
        }

        // Closing the drawer after a choice returns the full width to the main column.
        let open = layout.present(800., true);
        assert_eq!(open.reserved_left, open.sidebar);
        let closed = layout.present(800., false);
        assert_eq!(closed.mode, LayoutMode::Drawer);
        assert!(!closed.drawer_open);
        assert_eq!(closed.reserved_left, 0);

        let panel = layout.drag(1000., LayoutColumn::Panel, 320, 80.);
        assert!(panel.custom);
        assert_eq!(panel.panel, 400);
        assert_eq!(
            panel.sidebar, 240,
            "panel drag snapshots the compact sidebar"
        );
        assert_eq!(panel.present(1000., false).reserved_left, 240);
        assert_eq!(panel.present(1000., false).panel, 400);

        let both = panel.drag(1000., LayoutColumn::Sidebar, 240, 30.);
        assert_eq!((both.sidebar, both.panel), (270, 400));
        assert_eq!(
            layout
                .drag(1000., LayoutColumn::Sidebar, 240, 10_000.)
                .sidebar,
            WorkbenchLayout::SIDEBAR_MAX
        );
        assert_eq!(
            layout
                .drag(1488., LayoutColumn::Sidebar, 260, -10_000.)
                .sidebar,
            WorkbenchLayout::SIDEBAR_MIN
        );
        assert_eq!(
            layout.drag(1488., LayoutColumn::Panel, 320, 10_000.).panel,
            WorkbenchLayout::PANEL_MAX
        );
        assert_eq!(
            layout.drag(1488., LayoutColumn::Panel, 320, -10_000.).panel,
            WorkbenchLayout::PANEL_MIN
        );
        assert_eq!(
            layout.drag(1488., LayoutColumn::Panel, 320, f32::NAN),
            layout
        );
        assert_eq!(layout.drag(1000., LayoutColumn::Sidebar, 240, 0.2), layout);

        let restored = both.restore();
        assert_eq!(restored, WorkbenchLayout::default());
        assert!(restored.valid());
        assert_eq!(restored.present(1000., false).reserved_left, 240);
        assert_eq!(restored.present(1488., false).sidebar, 260);

        let saved =
            serde_json::from_value::<WorkbenchLayout>(serde_json::to_value(both).unwrap()).unwrap();
        assert_eq!(saved, both);
        assert!(both.valid());
        assert!(
            !WorkbenchLayout {
                sidebar: 199,
                panel: 320,
                custom: true
            }
            .valid()
        );
        assert!(
            !WorkbenchLayout {
                sidebar: 240,
                panel: 320,
                custom: false
            }
            .valid()
        );
        assert!(WorkbenchLayout::is_unmodified(&WorkbenchLayout::default()));
        assert!(!WorkbenchLayout::is_unmodified(&both));
    }
}
