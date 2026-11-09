//! Git flow configuration (M7-06, ADR-037): git-flow's keys in the
//! repository config, so repositories initialized by the `git-flow` CLI,
//! GitKraken or Gitloom are interchangeable. Reads come from the resolved
//! config (gix), writes go through `git config`.

use crate::{check_branch_name, Error, GitWriter, Result, RunOptions};

const PRODUCTION: &str = "gitflow.branch.master";
const DEVELOPMENT: &str = "gitflow.branch.develop";
const FEATURE: &str = "gitflow.prefix.feature";
const RELEASE: &str = "gitflow.prefix.release";
const HOTFIX: &str = "gitflow.prefix.hotfix";
const VERSION_TAG: &str = "gitflow.prefix.versiontag";

/// Branch types Gitloom starts and finishes (AVH's `bugfix/` and `support/`
/// are read, kept and not shown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlowKind {
    Feature,
    Release,
    Hotfix,
}

impl FlowKind {
    pub const ALL: [Self; 3] = [Self::Feature, Self::Release, Self::Hotfix];

    /// "feature", "release", "hotfix".
    pub fn noun(self) -> &'static str {
        match self {
            Self::Feature => "feature",
            Self::Release => "release",
            Self::Hotfix => "hotfix",
        }
    }
}

/// A local branch of a Git flow type: `feature/login` → (Feature, "login").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowBranch {
    pub kind: FlowKind,
    /// Name without the prefix (a version for releases and hotfixes).
    pub name: String,
}

/// git-flow's configuration of a repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowConfig {
    /// Production branch (`gitflow.branch.master`).
    pub production: String,
    /// Development branch (`gitflow.branch.develop`).
    pub development: String,
    pub feature: String,
    pub release: String,
    pub hotfix: String,
    /// Prefix of release and hotfix tags (`gitflow.prefix.versiontag`).
    pub version_tag: String,
}

impl FlowConfig {
    /// Defaults of the initialization: production `main` if it exists,
    /// otherwise `master`; `develop`; git-flow's prefixes; no tag prefix.
    pub fn defaults(local_branches: &[&str]) -> Self {
        let production = if local_branches.contains(&"main") {
            "main"
        } else {
            "master"
        };
        Self {
            production: production.into(),
            development: "develop".into(),
            feature: "feature/".into(),
            release: "release/".into(),
            hotfix: "hotfix/".into(),
            version_tag: String::new(),
        }
    }

    /// Configuration from config values (`get(key)`); `None` unless both
    /// branch keys are set (the CLI's "initialized" check). Missing
    /// prefixes take git-flow's defaults.
    pub fn from_values(get: impl Fn(&str) -> Option<String>) -> Option<Self> {
        let production = get(PRODUCTION).filter(|v| !v.is_empty())?;
        let development = get(DEVELOPMENT).filter(|v| !v.is_empty())?;
        let prefix = |key, default: &str| get(key).unwrap_or_else(|| default.to_owned());
        Some(Self {
            production,
            development,
            feature: prefix(FEATURE, "feature/"),
            release: prefix(RELEASE, "release/"),
            hotfix: prefix(HOTFIX, "hotfix/"),
            version_tag: prefix(VERSION_TAG, ""),
        })
    }

    pub fn prefix(&self, kind: FlowKind) -> &str {
        match kind {
            FlowKind::Feature => &self.feature,
            FlowKind::Release => &self.release,
            FlowKind::Hotfix => &self.hotfix,
        }
    }

    /// Full branch name of `name` of type `kind`.
    pub fn branch_name(&self, kind: FlowKind, name: &str) -> String {
        format!("{}{name}", self.prefix(kind))
    }

    /// Type and name of a local branch, if it belongs to Git flow. The
    /// longest matching prefix wins (prefixes may nest).
    pub fn classify(&self, branch: &str) -> Option<FlowBranch> {
        FlowKind::ALL
            .into_iter()
            .filter(|kind| !self.prefix(*kind).is_empty())
            .filter_map(|kind| {
                let name = branch.strip_prefix(self.prefix(kind))?;
                (!name.is_empty()).then(|| (self.prefix(kind).len(), kind, name))
            })
            .max_by_key(|(len, _, _)| *len)
            .map(|(_, kind, name)| FlowBranch {
                kind,
                name: name.to_owned(),
            })
    }

    /// Why the configuration cannot be saved, if it cannot.
    pub fn problem(&self) -> Option<String> {
        for (label, name) in [
            ("Production branch", &self.production),
            ("Development branch", &self.development),
        ] {
            if let Some(problem) = check_branch_name(name) {
                return Some(format!("{label}: {problem}"));
            }
        }
        if self.production == self.development {
            return Some("Production and development must be different branches".into());
        }
        for (label, prefix) in [
            ("Feature prefix", &self.feature),
            ("Release prefix", &self.release),
            ("Hotfix prefix", &self.hotfix),
        ] {
            if prefix.is_empty() {
                return Some(format!("{label}: enter a prefix, e.g. \"feature/\""));
            }
            // A prefix is fine if a branch can start with it.
            if check_branch_name(&format!("{prefix}x")).is_some() {
                return Some(format!("{label}: \"{prefix}\" cannot start a branch name"));
            }
        }
        if !self.version_tag.is_empty()
            && check_branch_name(&format!("{}1", self.version_tag)).is_some()
        {
            return Some(format!(
                "Version tag prefix: \"{}\" cannot start a tag name",
                self.version_tag
            ));
        }
        None
    }

    fn entries(&self) -> [(&'static str, &str); 6] {
        [
            (PRODUCTION, &self.production),
            (DEVELOPMENT, &self.development),
            (FEATURE, &self.feature),
            (RELEASE, &self.release),
            (HOTFIX, &self.hotfix),
            (VERSION_TAG, &self.version_tag),
        ]
    }
}

/// Writes the configuration to the repository config; other `gitflow.*`
/// keys (bugfix, support, AVH options) stay.
pub fn write(writer: &dyn GitWriter, config: &FlowConfig) -> Result<()> {
    if let Some(problem) = config.problem() {
        return Err(Error::Backend(problem));
    }
    for (key, value) in config.entries() {
        writer.run(&["config", "--local", key, value], RunOptions::default())?;
    }
    Ok(())
}

/// Removes every `gitflow` section from the repository config (branches
/// are never touched).
pub fn remove(writer: &dyn GitWriter) -> Result<()> {
    let read = RunOptions {
        read_only: true,
        ..RunOptions::default()
    };
    // Exit code 1: no gitflow key at all.
    let listed = writer.run_status(
        &[
            "config",
            "--local",
            "--name-only",
            "--get-regexp",
            r"^gitflow\.",
        ],
        read,
    )?;
    let mut sections: Vec<String> = Vec::new();
    for key in listed.1.stdout_text().lines() {
        // gitflow.branch.master → gitflow.branch; gitflow.x → gitflow
        let section = match key.rsplit_once('.') {
            Some((section, _)) => section.to_owned(),
            None => continue,
        };
        if !sections.contains(&section) {
            sections.push(section);
        }
    }
    for section in sections {
        writer.run(
            &["config", "--local", "--remove-section", &section],
            RunOptions::default(),
        )?;
    }
    Ok(())
}

/// Initializes Git flow: writes the configuration and creates missing
/// branches locally (production from HEAD, development from the
/// production tip). HEAD stays where it is.
pub fn init(writer: &dyn GitWriter, config: &FlowConfig) -> Result<()> {
    if !writer.has_head()? {
        return Err(Error::Backend(
            "Git flow needs at least one commit; commit something first".into(),
        ));
    }
    if let Some(problem) = config.problem() {
        return Err(Error::Backend(problem));
    }
    let exists = |branch: &str| -> Result<bool> {
        let read = RunOptions {
            read_only: true,
            ..RunOptions::default()
        };
        let reference = format!("refs/heads/{branch}");
        Ok(writer
            .run_status(&["rev-parse", "--verify", "--quiet", &reference], read)?
            .0
            == Some(0))
    };
    if !exists(&config.production)? {
        writer.run(
            &["branch", &config.production, "HEAD"],
            RunOptions::default(),
        )?;
    }
    if !exists(&config.development)? {
        writer.run(
            &["branch", &config.development, &config.production],
            RunOptions::default(),
        )?;
    }
    write(writer, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> FlowConfig {
        FlowConfig::defaults(&["main", "develop"])
    }

    #[test]
    fn defaults_prefer_main() {
        assert_eq!(config().production, "main");
        assert_eq!(FlowConfig::defaults(&["master"]).production, "master");
        assert_eq!(FlowConfig::defaults(&[]).production, "master");
        assert_eq!(config().development, "develop");
        assert_eq!(config().version_tag, "");
    }

    #[test]
    fn from_values_needs_both_branches_and_fills_prefixes() {
        let values = |pairs: &'static [(&str, &str)]| {
            move |key: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| (*v).to_owned())
            }
        };
        assert_eq!(FlowConfig::from_values(values(&[])), None);
        assert_eq!(
            FlowConfig::from_values(values(&[(PRODUCTION, "master")])),
            None
        );
        let parsed = FlowConfig::from_values(values(&[
            (PRODUCTION, "master"),
            (DEVELOPMENT, "dev"),
            (FEATURE, "feat/"),
            (VERSION_TAG, "v"),
        ]))
        .unwrap();
        assert_eq!(parsed.development, "dev");
        assert_eq!(parsed.feature, "feat/");
        assert_eq!(parsed.release, "release/", "default");
        assert_eq!(parsed.version_tag, "v");
    }

    #[test]
    fn classifies_flow_branches() {
        let config = config();
        assert_eq!(
            config.classify("feature/login"),
            Some(FlowBranch {
                kind: FlowKind::Feature,
                name: "login".into()
            })
        );
        assert_eq!(
            config.classify("release/1.2.0").map(|b| b.kind),
            Some(FlowKind::Release)
        );
        assert_eq!(
            config.classify("hotfix/1.2.1").map(|b| b.name),
            Some("1.2.1".into())
        );
        assert_eq!(config.classify("feature/"), None, "prefix alone");
        assert_eq!(config.classify("develop"), None);
        assert_eq!(config.classify("bugfix/x"), None, "AVH types are not shown");
        // Nested prefixes: the longest wins.
        let nested = FlowConfig {
            feature: "f/".into(),
            hotfix: "f/hot/".into(),
            ..config
        };
        assert_eq!(
            nested.classify("f/hot/1.0").map(|b| b.kind),
            Some(FlowKind::Hotfix)
        );
        assert_eq!(nested.branch_name(FlowKind::Feature, "x"), "f/x");
    }

    #[test]
    fn validation() {
        assert_eq!(config().problem(), None);
        let same = FlowConfig {
            development: "main".into(),
            ..config()
        };
        assert!(same.problem().unwrap().contains("different"));
        let bad_branch = FlowConfig {
            development: "dev elop".into(),
            ..config()
        };
        assert!(bad_branch
            .problem()
            .unwrap()
            .starts_with("Development branch"));
        let bad_prefix = FlowConfig {
            feature: "feat..".into(),
            ..config()
        };
        assert!(bad_prefix.problem().unwrap().starts_with("Feature prefix"));
        let empty_prefix = FlowConfig {
            release: String::new(),
            ..config()
        };
        assert!(empty_prefix.problem().is_some());
        // A prefix without "/" is accepted like git-flow does.
        let plain = FlowConfig {
            feature: "feat-".into(),
            ..config()
        };
        assert_eq!(plain.problem(), None);
        let tag = FlowConfig {
            version_tag: "v~".into(),
            ..config()
        };
        assert!(tag.problem().unwrap().starts_with("Version tag prefix"));
    }
}
