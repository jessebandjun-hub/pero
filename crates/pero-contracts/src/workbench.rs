//! Preferences shared by Desktop and the Host-owned settings store.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    Light,
    Dark,
    #[default]
    System,
}

impl Appearance {
    pub fn dark(self, system_dark: bool) -> bool {
        match self {
            Self::Light => false,
            Self::Dark => true,
            Self::System => system_dark,
        }
    }
    pub fn label_key(self) -> &'static str {
        match self {
            Self::Light => "appearance.light",
            Self::Dark => "appearance.dark",
            Self::System => "appearance.system",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectBinding {
    pub workspace: String,
    pub project_id: String,
}

/// Sidebar width saved by the Host. Narrow windows keep a compact sidebar
/// instead of applying this value, so a wide preference cannot cover the main column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkbenchLayout {
    pub sidebar: u32,
}

impl Default for WorkbenchLayout {
    fn default() -> Self {
        Self {
            sidebar: Self::SIDEBAR_DEFAULT,
        }
    }
}

impl WorkbenchLayout {
    pub const SIDEBAR_DEFAULT: u32 = 260;
    pub const SIDEBAR_MIN: u32 = 200;
    pub const SIDEBAR_MAX: u32 = 420;
    pub const COMPACT_VIEWPORT: f32 = 1150.;
    pub const COMPACT_SIDEBAR: u32 = 240;

    pub fn valid(self) -> bool {
        (Self::SIDEBAR_MIN..=Self::SIDEBAR_MAX).contains(&self.sidebar)
    }

    pub fn clamp_sidebar(px: f32) -> u32 {
        px.round()
            .clamp(Self::SIDEBAR_MIN as f32, Self::SIDEBAR_MAX as f32) as u32
    }

    pub fn sidebar_px(self, viewport_width: f32) -> f32 {
        if viewport_width < Self::COMPACT_VIEWPORT {
            Self::COMPACT_SIDEBAR as f32
        } else {
            self.sidebar as f32
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkbenchPreferences {
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub projects: Vec<ProjectBinding>,
    #[serde(default)]
    pub layout: WorkbenchLayout,
}

impl WorkbenchPreferences {
    pub fn valid(&self) -> bool {
        self.layout.valid()
            && self.projects.len() <= 128
            && self.projects.iter().enumerate().all(|(i, binding)| {
                !binding.workspace.trim().is_empty()
                    && binding.workspace.len() <= 4096
                    && !binding.workspace.contains('\0')
                    && binding.project_id.len() <= 128
                    && !binding.project_id.chars().any(char::is_control)
                    && !self.projects[..i]
                        .iter()
                        .any(|other| other.workspace == binding.workspace)
            })
    }

    pub fn bind(&mut self, workspace: String, project_id: String) {
        self.projects
            .retain(|binding| binding.workspace != workspace);
        self.projects.push(ProjectBinding {
            workspace,
            project_id,
        });
    }
}

#[derive(Deserialize)]
#[serde(tag = "method", deny_unknown_fields)]
pub enum WorkbenchSettingsRequest {
    #[serde(rename = "settings.workbench.get")]
    Get,
    #[serde(rename = "settings.workbench")]
    Set { preferences: WorkbenchPreferences },
}

/// Repository identity used to reconcile a checkout with its shared Project.
/// Transport spelling (SSH vs HTTPS, `.git`, case) is not part of identity.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct RepositoryIdentity {
    pub host: String,
    pub owner: String,
    pub name: String,
}

impl RepositoryIdentity {
    pub fn from_git_remote(remote: &str) -> Option<Self> {
        let remote = remote.trim();
        let (authority, path) = if let Some((scheme, location)) = remote.split_once("://") {
            if !matches!(scheme, "https" | "http" | "ssh" | "git") {
                return None;
            }
            location.split_once('/')?
        } else {
            let (authority, path) = remote.split_once(':')?;
            if !authority.contains('@') {
                return None;
            }
            (authority, path)
        };
        let host = authority.rsplit('@').next()?.to_ascii_lowercase();
        let path = path.trim_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        let (owner, name) = path.rsplit_once('/')?;
        if host.is_empty()
            || owner.is_empty()
            || name.is_empty()
            || [&host, owner, name].into_iter().any(|part| {
                part.chars()
                    .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '?' | '#' | '\\'))
            })
        {
            return None;
        }
        Some(Self {
            host,
            owner: owner.to_ascii_lowercase(),
            name: name.to_ascii_lowercase(),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceIdentity {
    pub repository: Option<RepositoryIdentity>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct WorkspaceRepository {
    pub workspace: String,
    pub repository: RepositoryIdentity,
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn saved_sidebar_applies_when_wide_and_stays_compact_when_narrow() {
        let saved = WorkbenchLayout { sidebar: 320 };
        for (viewport, width) in [(1586., 320.), (1150., 320.), (1149., 240.), (1000., 240.)] {
            assert_eq!(saved.sidebar_px(viewport), width, "{viewport}");
        }
        for (sidebar, accepted) in [
            (200, true),
            (260, true),
            (420, true),
            (199, false),
            (421, false),
        ] {
            assert_eq!(WorkbenchLayout { sidebar }.valid(), accepted, "{sidebar}");
        }
        for (px, sidebar) in [(10., 200), (900., 420), (320.4, 320), (199.6, 200)] {
            assert_eq!(WorkbenchLayout::clamp_sidebar(px), sidebar, "{px}");
        }
        let parsed: WorkbenchPreferences =
            serde_json::from_str(r#"{"appearance":"dark","projects":[]}"#).unwrap();
        assert_eq!(parsed.layout, WorkbenchLayout::default());
        assert!(parsed.valid());
    }
}

#[cfg(test)]
mod repository_tests {
    use super::*;
    #[test]
    fn repository_identity_normalizes_transport_but_retains_owner_and_host() {
        let canonical =
            RepositoryIdentity::from_git_remote("git@github.com:phodal/pero.git").unwrap();
        for (remote, same) in [
            ("https://github.com/Phodal/Pero.git", true),
            ("ssh://git@github.com/phodal/pero", true),
            ("https://github.com/other/pero", false),
            ("git@gitlab.com:phodal/pero.git", false),
            ("/local/pero", false),
            ("file:///local/pero", false),
            ("https://github.com/phodal/pero?token=secret", false),
        ] {
            assert_eq!(
                RepositoryIdentity::from_git_remote(remote).as_ref() == Some(&canonical),
                same,
                "{remote}"
            );
        }
    }
}
