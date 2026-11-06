//! Opt-in update check (M6-06, ADR-030). Off by default; when on, at most
//! once a day the release list of the GitHub repository is requested and a
//! newer version is offered in the status bar. Nothing is downloaded or
//! installed. With the setting off no HTTP client exists.

use std::cmp::Ordering;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Time between two checks.
pub const INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Semantic version (`1.2.3` or `1.2.3-rc.1`; build metadata ignored).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// Dot-separated pre-release identifiers; empty for a release.
    pub pre: Vec<String>,
}

impl Version {
    /// Parses `1.2.3`, `v1.2.3-rc.1` or `1.2.3+build`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = text.strip_prefix('v').unwrap_or(text);
        let text = text.split('+').next()?;
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };
        let mut numbers = core.split('.').map(|n| {
            (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                .then(|| n.parse::<u64>().ok())
                .flatten()
        });
        let (major, minor, patch) = (numbers.next()??, numbers.next()??, numbers.next()??);
        if numbers.next().is_some() {
            return None;
        }
        let pre = match pre {
            Some(pre) => {
                let ids: Vec<String> = pre.split('.').map(str::to_owned).collect();
                if ids.iter().any(String::is_empty) {
                    return None;
                }
                ids
            }
            None => Vec::new(),
        };
        Some(Self {
            major,
            minor,
            patch,
            pre,
        })
    }

    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if self.is_prerelease() {
            write!(f, "-{}", self.pre.join("."))?;
        }
        Ok(())
    }
}

/// SemVer 2.0 precedence of two pre-release identifiers.
fn compare_identifier(a: &str, b: &str) -> Ordering {
    let numeric = |s: &str| s.bytes().all(|c| c.is_ascii_digit());
    match (numeric(a), numeric(b)) {
        (true, true) => a
            .parse::<u64>()
            .unwrap_or(0)
            .cmp(&b.parse::<u64>().unwrap_or(0)),
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => a.cmp(b),
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                // A release ranks above its pre-releases.
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => {
                    for (a, b) in self.pre.iter().zip(&other.pre) {
                        let order = compare_identifier(a, b);
                        if order != Ordering::Equal {
                            return order;
                        }
                    }
                    self.pre.len().cmp(&other.pre.len())
                }
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A published release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// Release page in the browser.
    pub url: String,
}

/// Where releases come from (GitHub; a fake in tests).
pub trait ReleaseSource: Send + Sync {
    fn releases(&self) -> Result<Vec<Release>, String>;
}

/// The newest release that is newer than `running`. Pre-releases count
/// only when `running` is one itself.
pub fn newer_release(running: &Version, releases: &[Release]) -> Option<Release> {
    releases
        .iter()
        .filter(|r| !r.version.is_prerelease() || running.is_prerelease())
        .filter(|r| r.version > *running)
        .max_by(|a, b| a.version.cmp(&b.version))
        .cloned()
}

/// State stored in the session file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateState {
    /// Unix time (seconds) of the last check.
    pub last_check: Option<u64>,
    /// Version whose notice the user dismissed.
    pub dismissed: Option<String>,
}

impl UpdateState {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Unix time in seconds.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Seconds until the next check is due (0: now).
pub fn due_in(last_check: Option<u64>, now: u64) -> u64 {
    match last_check {
        // A clock that went backwards counts as due.
        Some(last) if last <= now => INTERVAL.as_secs().saturating_sub(now - last),
        _ => 0,
    }
}

/// True if the notice for `release` should show (not dismissed).
pub fn should_notify(release: &Release, state: &UpdateState) -> bool {
    state.dismissed.as_deref() != Some(release.version.to_string().as_str())
}

/// Release list URL of the GitHub API for the repository URL
/// `https://github.com/<owner>/<repo>`.
pub fn api_url(repository_url: &str) -> Option<String> {
    let path = repository_url
        .strip_prefix("https://github.com/")?
        .trim_end_matches('/');
    let (owner, repo) = path.split_once('/')?;
    (!owner.is_empty() && !repo.is_empty() && !repo.contains('/'))
        .then(|| format!("https://api.github.com/repos/{owner}/{repo}/releases?per_page=30"))
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
}

/// Releases from the GitHub API response (drafts and unparsable tags
/// skipped). The version comes from the tag; GitHub's `prerelease` flag is
/// not needed because the version says it.
pub fn parse_api(json: &str) -> Result<Vec<Release>, String> {
    let list: Vec<ApiRelease> =
        serde_json::from_str(json).map_err(|e| format!("unexpected answer: {e}"))?;
    Ok(list
        .into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            Some(Release {
                version: Version::parse(&r.tag_name)?,
                url: r.html_url,
            })
        })
        .collect())
}

/// Releases over HTTPS (rustls, proxy from the environment, 10 s timeout).
pub struct GitHubSource {
    agent: ureq::Agent,
    url: String,
}

impl GitHubSource {
    pub fn new() -> Option<Self> {
        let url = api_url(git_core::identity::REPOSITORY_URL)?;
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .user_agent(git_core::identity::user_agent(env!("CARGO_PKG_VERSION")))
            .build();
        Some(Self {
            agent: ureq::Agent::new_with_config(config),
            url,
        })
    }
}

impl ReleaseSource for GitHubSource {
    fn releases(&self) -> Result<Vec<Release>, String> {
        let mut response = self
            .agent
            .get(&self.url)
            .header("Accept", "application/vnd.github+json")
            .call()
            .map_err(|e| e.to_string())?;
        let body = response
            .body_mut()
            .with_config()
            .limit(2 * 1024 * 1024)
            .read_to_string()
            .map_err(|e| e.to_string())?;
        parse_api(&body)
    }
}

/// Source used by the app; `None` if the repository URL is not on GitHub.
pub fn github_source() -> Option<std::sync::Arc<dyn ReleaseSource>> {
    GitHubSource::new().map(|s| std::sync::Arc::new(s) as std::sync::Arc<dyn ReleaseSource>)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    fn release(tag: &str) -> Release {
        Release {
            version: v(tag),
            url: format!("https://example.com/{tag}"),
        }
    }

    #[test]
    fn parse_versions() {
        assert_eq!(v("1.2.3").to_string(), "1.2.3");
        assert_eq!(v("v1.0.0-rc.1").to_string(), "1.0.0-rc.1");
        assert_eq!(v("1.0.0+build.5").to_string(), "1.0.0");
        for bad in [
            "",
            "1.2",
            "1.2.3.4",
            "a.b.c",
            "1.2.3-",
            "1.2.3-rc..1",
            "1.-2.3",
        ] {
            assert!(Version::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn semver_precedence() {
        // The ordering example of the SemVer 2.0 specification.
        let ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
            "1.1.0",
            "2.0.0",
        ];
        for pair in ordered.windows(2) {
            assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
        }
        assert!(v("1.0.0-rc.2") < v("1.0.0-rc.10"));
        assert_eq!(v("1.0.0").cmp(&v("v1.0.0+x")), Ordering::Equal);
    }

    #[test]
    fn prereleases_only_for_prerelease_builds() {
        let releases = [release("1.0.0"), release("1.1.0-rc.1"), release("0.9.0")];
        // A release build ignores the rc.
        assert_eq!(
            newer_release(&v("1.0.0"), &releases),
            None,
            "1.1.0-rc.1 is not offered to 1.0.0"
        );
        assert_eq!(
            newer_release(&v("0.9.0"), &releases).unwrap().version,
            v("1.0.0")
        );
        // A pre-release build sees later rcs and the final release.
        assert_eq!(
            newer_release(&v("1.1.0-rc.0"), &releases).unwrap().version,
            v("1.1.0-rc.1")
        );
        let with_final = [
            release("1.0.0-rc.1"),
            release("1.0.0-rc.2"),
            release("1.0.0"),
        ];
        assert_eq!(
            newer_release(&v("1.0.0-rc.1"), &with_final)
                .unwrap()
                .version,
            v("1.0.0")
        );
        assert_eq!(
            newer_release(&v("1.0.0-rc.1"), &with_final[..2])
                .unwrap()
                .version,
            v("1.0.0-rc.2")
        );
        assert_eq!(newer_release(&v("1.0.0"), &with_final), None);
    }

    #[test]
    fn checks_at_most_once_a_day() {
        let day = INTERVAL.as_secs();
        assert_eq!(due_in(None, 1000), 0);
        assert_eq!(due_in(Some(1000), 1000), day);
        assert_eq!(due_in(Some(1000), 1000 + day - 60), 60);
        assert_eq!(due_in(Some(1000), 1000 + day), 0);
        assert_eq!(due_in(Some(1000), 1000 + 3 * day), 0);
        // Clock went backwards.
        assert_eq!(due_in(Some(5000), 1000), 0);
    }

    #[test]
    fn dismiss_is_per_version() {
        let mut state = UpdateState::default();
        assert!(should_notify(&release("1.1.0"), &state));
        state.dismissed = Some("1.1.0".into());
        assert!(!should_notify(&release("v1.1.0"), &state));
        assert!(should_notify(&release("1.2.0"), &state));
    }

    /// Manual check of the real API (network; the repository itself is
    /// private until M6-14, so a public one is used).
    #[test]
    #[ignore = "network"]
    fn live_github_release_list() {
        let mut source = GitHubSource::new().unwrap();
        source.url = api_url("https://github.com/iced-rs/iced").unwrap();
        let releases = source.releases().unwrap();
        assert!(releases.iter().any(|r| r.version == v("0.13.0")));
    }

    #[test]
    fn github_api_answer() {
        assert_eq!(
            api_url("https://github.com/owner/repo").as_deref(),
            Some("https://api.github.com/repos/owner/repo/releases?per_page=30")
        );
        assert!(api_url("https://gitlab.com/owner/repo").is_none());
        assert!(api_url("https://github.com/owner").is_none());
        assert!(api_url(git_core::identity::REPOSITORY_URL).is_some());
        let json = r#"[
            {"tag_name": "v1.1.0", "html_url": "https://github.com/o/r/releases/tag/v1.1.0", "draft": false, "prerelease": false},
            {"tag_name": "v1.2.0", "html_url": "x", "draft": true},
            {"tag_name": "nightly", "html_url": "y"},
            {"tag_name": "v1.1.0-rc.1", "html_url": "z", "prerelease": true}
        ]"#;
        let releases = parse_api(json).unwrap();
        assert_eq!(releases.len(), 2);
        assert_eq!(releases[0].version, v("1.1.0"));
        assert!(releases[0].url.ends_with("/tag/v1.1.0"));
        assert!(parse_api("{\"message\": \"rate limited\"}").is_err());
    }
}
