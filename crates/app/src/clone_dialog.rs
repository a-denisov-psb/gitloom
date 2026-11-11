//! "Clone a repository" dialog (M10-02): state and validation. No I/O here;
//! the target folder is checked by the clone itself in the background.

use std::path::PathBuf;

use crate::app::TabId;

/// A running clone: Cancel stops it; the latest progress is shown.
#[derive(Debug, Clone)]
pub struct CloneRun {
    pub cancel: git_core::CancelToken,
    pub progress: Option<git_core::Progress>,
}

#[derive(Debug, Clone)]
pub struct CloneDialog {
    /// Tab the dialog was opened from; the clone opens there.
    pub tab: TabId,
    pub url: String,
    /// "Where to clone to": the parent folder.
    pub parent: String,
    /// Folder name, derived from the URL until edited.
    pub name: String,
    name_edited: bool,
    /// `--recurse-submodules`.
    pub submodules: bool,
    pub error: Option<String>,
    pub running: Option<CloneRun>,
}

impl CloneDialog {
    pub fn new(tab: TabId, parent: String) -> Self {
        Self {
            tab,
            url: String::new(),
            parent,
            name: String::new(),
            name_edited: false,
            submodules: true,
            error: None,
            running: None,
        }
    }

    pub fn set_url(&mut self, url: String) {
        if !self.name_edited {
            self.name = git_core::clone::repo_name(&url).unwrap_or_default();
        }
        self.url = url;
        self.error = None;
    }

    /// An emptied name follows the URL again.
    pub fn set_name(&mut self, name: String) {
        self.name_edited = !name.trim().is_empty();
        self.name = if self.name_edited {
            name
        } else {
            git_core::clone::repo_name(&self.url).unwrap_or_default()
        };
        self.error = None;
    }

    pub fn set_parent(&mut self, parent: String) {
        self.parent = parent;
        self.error = None;
    }

    /// The folder the repository is cloned into, once parent and name are set.
    pub fn full_path(&self) -> Option<PathBuf> {
        let (parent, name) = (self.parent.trim(), self.name.trim());
        (!parent.is_empty() && !name.is_empty()).then(|| PathBuf::from(parent).join(name))
    }

    /// What is missing before Clone can start (checked without I/O).
    pub fn problem(&self) -> Option<&'static str> {
        let name = self.name.trim();
        if self.url.trim().is_empty() {
            Some("Enter the URL of the repository")
        } else if self.parent.trim().is_empty() {
            Some("Choose where to clone to")
        } else if name.is_empty() {
            Some("Enter a name for the folder")
        } else if name.contains(['/', '\\']) || name == "." || name == ".." {
            Some("The name cannot contain / or \\")
        } else {
            None
        }
    }

    /// "Receiving objects 45 %" while running.
    pub fn progress_text(&self) -> Option<String> {
        let run = self.running.as_ref()?;
        Some(match &run.progress {
            Some(git_core::Progress {
                phase,
                percent: Some(percent),
            }) => format!("{phase} {percent} %"),
            Some(progress) => format!("{}…", progress.phase),
            None => "Connecting…".to_owned(),
        })
    }
}

/// Default "Where to clone to": the folder of the most recent repository,
/// else the home folder.
pub fn default_parent(recent: &[crate::session::RecentRepo]) -> String {
    recent
        .first()
        .and_then(|r| r.path.parent())
        .map(PathBuf::from)
        .or_else(|| directories::UserDirs::new().map(|d| d.home_dir().to_path_buf()))
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog() -> CloneDialog {
        CloneDialog::new(TabId::for_tests(1), "/home/ana/projects".into())
    }

    #[test]
    fn name_follows_the_url_until_edited() {
        let mut d = dialog();
        d.set_url("git@github.com:owner/tool.git".into());
        assert_eq!(d.name, "tool");
        assert_eq!(
            d.full_path(),
            Some(PathBuf::from("/home/ana/projects").join("tool"))
        );
        d.set_name("my-tool".into());
        d.set_url("https://example.com/other.git".into());
        assert_eq!(d.name, "my-tool", "an edited name stays");
        d.set_name(String::new());
        assert_eq!(d.name, "other", "an emptied name follows the URL again");
    }

    #[test]
    fn validation_without_io() {
        let mut d = dialog();
        assert_eq!(d.problem(), Some("Enter the URL of the repository"));
        d.set_url("https://example.com/".into());
        assert_eq!(d.problem(), Some("Enter a name for the folder"));
        d.set_name("a/b".into());
        assert_eq!(d.problem(), Some("The name cannot contain / or \\"));
        d.set_name("repo".into());
        d.set_parent(" ".into());
        assert_eq!(d.problem(), Some("Choose where to clone to"));
        assert_eq!(d.full_path(), None);
        d.set_parent("/tmp".into());
        assert_eq!(d.problem(), None);
    }

    #[test]
    fn progress_text() {
        let mut d = dialog();
        assert_eq!(d.progress_text(), None);
        d.running = Some(CloneRun {
            cancel: git_core::CancelToken::new(),
            progress: None,
        });
        assert_eq!(d.progress_text().as_deref(), Some("Connecting…"));
        if let Some(run) = d.running.as_mut() {
            run.progress = Some(git_core::Progress {
                phase: "Receiving objects".into(),
                percent: Some(45),
            });
        }
        assert_eq!(d.progress_text().as_deref(), Some("Receiving objects 45 %"));
    }
}
