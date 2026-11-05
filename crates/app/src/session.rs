//! Persisted session: recent repositories, open tabs and pane ratios.
//!
//! All functions touching the file system are blocking; call them off the UI thread.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::panes::PaneRatios;

pub const VERSION: u32 = 1;
pub const MAX_RECENT: usize = 20;
const FILE_NAME: &str = "session.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentRepo {
    pub path: PathBuf,
    /// Unix timestamp (seconds) of the last time the repository was opened.
    pub last_opened: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub version: u32,
    /// Most recent first, at most [`MAX_RECENT`].
    pub recent: Vec<RecentRepo>,
    /// Repository roots of the open tabs (start tabs are not stored).
    pub tabs: Vec<PathBuf>,
    /// Index into `tabs`.
    pub active: usize,
    pub panes: PaneRatios,
    /// Mode of the toolbar Pull button (last one chosen in its dropdown).
    #[serde(default)]
    pub pull_mode: PullModeSetting,
    /// Hidden and soloed branches per repository root (M5-08).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub ref_filters: std::collections::BTreeMap<PathBuf, crate::ref_filter::RefFilter>,
}

/// [`git_core::PullMode`] as stored in the session file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PullModeSetting {
    #[default]
    FastForwardIfPossible,
    FastForwardOnly,
    Rebase,
}

impl From<PullModeSetting> for git_core::PullMode {
    fn from(mode: PullModeSetting) -> Self {
        match mode {
            PullModeSetting::FastForwardIfPossible => Self::FastForwardIfPossible,
            PullModeSetting::FastForwardOnly => Self::FastForwardOnly,
            PullModeSetting::Rebase => Self::Rebase,
        }
    }
}

impl From<git_core::PullMode> for PullModeSetting {
    fn from(mode: git_core::PullMode) -> Self {
        match mode {
            git_core::PullMode::FastForwardIfPossible => Self::FastForwardIfPossible,
            git_core::PullMode::FastForwardOnly => Self::FastForwardOnly,
            git_core::PullMode::Rebase => Self::Rebase,
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self {
            version: VERSION,
            recent: Vec::new(),
            tabs: Vec::new(),
            active: 0,
            panes: PaneRatios::default(),
            pull_mode: PullModeSetting::default(),
            ref_filters: Default::default(),
        }
    }
}

impl Session {
    /// Moves `path` to the front of the recent list.
    pub fn touch_recent(&mut self, path: &Path, now: i64) {
        self.recent.retain(|r| r.path != path);
        self.recent.insert(
            0,
            RecentRepo {
                path: path.to_path_buf(),
                last_opened: now,
            },
        );
        self.recent.truncate(MAX_RECENT);
    }

    pub fn remove_recent(&mut self, path: &Path) {
        self.recent.retain(|r| r.path != path);
    }

    pub fn is_recent(&self, path: &Path) -> bool {
        self.recent.iter().any(|r| r.path == path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Invalid(String),
    UnknownVersion(u64),
}

/// Parses a session file; unknown versions are rejected.
pub fn parse(text: &str) -> Result<Session, ParseError> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| ParseError::Invalid(e.to_string()))?;
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| ParseError::Invalid("missing version".into()))?;
    if version != u64::from(VERSION) {
        return Err(ParseError::UnknownVersion(version));
    }
    let mut session: Session =
        serde_json::from_value(value).map_err(|e| ParseError::Invalid(e.to_string()))?;
    session.recent.truncate(MAX_RECENT);
    session.panes = session.panes.sanitized();
    if session.active >= session.tabs.len() {
        session.active = 0;
    }
    Ok(session)
}

pub fn to_json(session: &Session) -> String {
    // Serializing plain data cannot fail; fall back to an empty object just in case.
    serde_json::to_string_pretty(session).unwrap_or_else(|_| "{}".into())
}

/// Result of loading the session at startup.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub session: Session,
    /// One-time message for the user (e.g. the file was corrupt and reset).
    pub notice: Option<String>,
}

/// Default location: `<config dir>/session.json`.
pub fn default_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", git_core::identity::DIR_NAME)
        .map(|dirs| dirs.config_dir().join(FILE_NAME))
}

/// Reads the session; a corrupt or unknown file is moved to `session.json.bak`.
pub fn load(path: &Path) -> Loaded {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Loaded {
                session: Session::default(),
                notice: None,
            }
        }
        Err(e) => {
            return Loaded {
                session: Session::default(),
                notice: Some(format!("Could not read the session file: {e}")),
            }
        }
    };
    match parse(&text) {
        Ok(session) => Loaded {
            session,
            notice: None,
        },
        Err(error) => {
            let backup = path.with_extension("json.bak");
            let moved = std::fs::rename(path, &backup).is_ok();
            let reason = match error {
                ParseError::Invalid(_) => "was unreadable".to_string(),
                ParseError::UnknownVersion(v) => format!("has unknown version {v}"),
            };
            let backup_note = if moved {
                format!(" A backup was saved as {}.", backup.display())
            } else {
                String::new()
            };
            Loaded {
                session: Session::default(),
                notice: Some(format!(
                    "The session file {reason} and was reset.{backup_note}"
                )),
            }
        }
    }
}

/// Writes atomically: temp file next to the target, then rename.
pub fn save(path: &Path, json: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, json)?;
    std::fs::rename(&temp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Session {
        let mut session = Session {
            tabs: vec![PathBuf::from("/a"), PathBuf::from("/b")],
            active: 1,
            panes: PaneRatios {
                sidebar: 0.2,
                center: 0.7,
            },
            ..Session::default()
        };
        session.touch_recent(Path::new("/a"), 10);
        session.touch_recent(Path::new("/b"), 20);
        let mut filter = crate::ref_filter::RefFilter::default();
        filter.toggle_hidden("topic", git_core::RefKind::LocalBranch);
        filter.toggle_soloed("origin/main", git_core::RefKind::RemoteBranch);
        session.ref_filters.insert(PathBuf::from("/a"), filter);
        session
    }

    #[test]
    fn round_trip() {
        let session = sample();
        assert_eq!(parse(&to_json(&session)), Ok(session));
    }

    #[test]
    fn json_contains_version() {
        assert!(to_json(&Session::default()).contains("\"version\": 1"));
    }

    #[test]
    fn recent_most_recent_first_without_duplicates() {
        let mut session = Session::default();
        session.touch_recent(Path::new("/a"), 1);
        session.touch_recent(Path::new("/b"), 2);
        session.touch_recent(Path::new("/a"), 3);
        let paths: Vec<_> = session.recent.iter().map(|r| r.path.clone()).collect();
        assert_eq!(paths, [PathBuf::from("/a"), PathBuf::from("/b")]);
        assert_eq!(session.recent[0].last_opened, 3);
    }

    #[test]
    fn recent_is_capped() {
        let mut session = Session::default();
        for i in 0..30 {
            session.touch_recent(&PathBuf::from(format!("/repo{i}")), i);
        }
        assert_eq!(session.recent.len(), MAX_RECENT);
        assert_eq!(session.recent[0].path, PathBuf::from("/repo29"));
        assert_eq!(
            session.recent[MAX_RECENT - 1].path,
            PathBuf::from("/repo10")
        );
    }

    #[test]
    fn remove_recent() {
        let mut session = sample();
        session.remove_recent(Path::new("/a"));
        assert!(!session.is_recent(Path::new("/a")));
        assert!(session.is_recent(Path::new("/b")));
    }

    #[test]
    fn unknown_version_is_rejected() {
        let text = to_json(&Session::default()).replace("\"version\": 1", "\"version\": 99");
        assert_eq!(parse(&text), Err(ParseError::UnknownVersion(99)));
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(matches!(parse("not json"), Err(ParseError::Invalid(_))));
        assert!(matches!(parse("{}"), Err(ParseError::Invalid(_))));
    }

    #[test]
    fn out_of_range_values_are_fixed() {
        let mut session = sample();
        session.active = 5;
        let parsed = parse(&to_json(&session)).unwrap();
        assert_eq!(parsed.active, 0);
    }

    #[test]
    fn load_missing_file_gives_default() {
        let dir =
            std::env::temp_dir().join(format!("gitloom-session-missing-{}", std::process::id()));
        let loaded = load(&dir.join("session.json"));
        assert_eq!(loaded.session, Session::default());
        assert!(loaded.notice.is_none());
    }

    #[test]
    fn corrupt_file_is_backed_up() {
        let dir =
            std::env::temp_dir().join(format!("gitloom-session-corrupt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.json");
        std::fs::write(&path, "{ broken").unwrap();

        let loaded = load(&path);
        assert_eq!(loaded.session, Session::default());
        assert!(loaded.notice.is_some());
        assert!(!path.exists());
        assert_eq!(
            std::fs::read_to_string(dir.join("session.json.bak")).unwrap(),
            "{ broken"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn save_then_load() {
        let dir = std::env::temp_dir().join(format!("gitloom-session-save-{}", std::process::id()));
        let path = dir.join("nested").join("session.json");
        let session = sample();
        save(&path, &to_json(&session)).unwrap();
        assert!(!path.with_extension("json.tmp").exists());
        assert_eq!(load(&path).session, session);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
