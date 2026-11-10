//! Git identity (`user.name` / `user.email`) of a repository (M8-02):
//! validation, the resolved values with their origin, and writes through
//! the `git` CLI (ADR-041: Git's configuration is the source of truth).

use crate::{Error, GitWriter, Result, RunOptions};

/// Name and email of a commit author.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identity {
    pub name: String,
    pub email: String,
}

/// Why `name` and `email` cannot form an identity (M8 decisions table):
/// name not empty; email not empty, contains `@`, no spaces, no `<`/`>`.
pub fn identity_problem(name: &str, email: &str) -> Option<&'static str> {
    let (name, email) = (name.trim(), email.trim());
    if name.is_empty() {
        return Some("Enter a name");
    }
    if name.contains(['<', '>', '\n']) {
        return Some("The name cannot contain < or >");
    }
    if email.is_empty() {
        return Some("Enter an email address");
    }
    if !email.contains('@') {
        return Some("The email address needs an @");
    }
    if email.contains(char::is_whitespace) {
        return Some("The email address cannot contain spaces");
    }
    if email.contains(['<', '>']) {
        return Some("The email address cannot contain < or >");
    }
    None
}

impl Identity {
    /// A valid identity from the trimmed values, or why it is not one.
    pub fn new(name: &str, email: &str) -> std::result::Result<Self, &'static str> {
        match identity_problem(name, email) {
            Some(problem) => Err(problem),
            None => Ok(Self {
                name: name.trim().to_owned(),
                email: email.trim().to_owned(),
            }),
        }
    }

    /// "Name <email>" as Git writes it.
    pub fn label(&self) -> String {
        format!("{} <{}>", self.name, self.email)
    }
}

/// Configuration file a value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigOrigin {
    /// The repository (or worktree) config.
    Local,
    /// `~/.gitconfig`, `$XDG_CONFIG_HOME/git/config` or `GIT_CONFIG_GLOBAL`.
    Global,
    /// System or Git installation config.
    System,
    /// Environment, command line or an unknown source.
    Other,
}

impl ConfigOrigin {
    /// "from the repository config" etc., for tooltips.
    pub fn description(self) -> &'static str {
        match self {
            Self::Local => "from the repository config",
            Self::Global => "from the global config",
            Self::System => "from the system config",
            Self::Other => "from the environment",
        }
    }
}

/// A configuration value with the file it comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValue {
    pub value: String,
    pub origin: ConfigOrigin,
}

/// What `user.name` and `user.email` resolve to in a repository (local over
/// global over system); `None` when unset or empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedIdentity {
    pub name: Option<ConfigValue>,
    pub email: Option<ConfigValue>,
}

impl ResolvedIdentity {
    /// The identity the next commit uses, if both values resolve.
    pub fn identity(&self) -> Option<Identity> {
        match (&self.name, &self.email) {
            (Some(name), Some(email)) => Some(Identity {
                name: name.value.clone(),
                email: email.value.clone(),
            }),
            _ => None,
        }
    }

    /// The repository config sets at least one of the values.
    pub fn has_local(&self) -> bool {
        [&self.name, &self.email]
            .iter()
            .any(|v| v.as_ref().is_some_and(|v| v.origin == ConfigOrigin::Local))
    }

    /// Origin shown for the identity: local if either value is local,
    /// else the origin of the name.
    pub fn origin(&self) -> Option<ConfigOrigin> {
        if self.has_local() {
            return Some(ConfigOrigin::Local);
        }
        self.name.as_ref().map(|v| v.origin)
    }
}

/// Configuration a write goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityScope {
    /// The repository config (`git config --local`).
    Local,
    /// The global config (`git config --global`).
    Global,
}

/// Writes `identity` as `user.name` and `user.email` to `scope`.
pub fn set_identity(
    writer: &dyn GitWriter,
    scope: IdentityScope,
    identity: &Identity,
) -> Result<()> {
    if let Some(problem) = identity_problem(&identity.name, &identity.email) {
        return Err(Error::Backend(problem.to_owned()));
    }
    let flag = match scope {
        IdentityScope::Local => "--local",
        IdentityScope::Global => "--global",
    };
    for (key, value) in [
        ("user.name", &identity.name),
        ("user.email", &identity.email),
    ] {
        writer.run(&["config", flag, key, value], RunOptions::default())?;
    }
    Ok(())
}

/// Removes `user.name` and `user.email` from the repository config, so the
/// global identity applies again. A missing key is not an error; a `[user]`
/// section left without keys is removed (no empty sections, see M6-20).
pub fn unset_local_identity(writer: &dyn GitWriter) -> Result<()> {
    for key in ["user.name", "user.email"] {
        let (code, output) = writer.run_status(
            &["config", "--local", "--unset-all", key],
            RunOptions::default(),
        )?;
        // 5: the key does not exist.
        if !matches!(code, Some(0) | Some(5)) {
            return Err(Error::Command {
                command: format!("git config --local --unset-all {key}"),
                code,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
    }
    let (code, output) = writer.run_status(
        &["config", "--local", "--get-regexp", r"^user\."],
        RunOptions {
            read_only: true,
            ..RunOptions::default()
        },
    )?;
    if code == Some(1) && output.stdout.is_empty() {
        // Exit code 128 when there is no section at all: nothing left over.
        let _ = writer.run_status(
            &["config", "--local", "--remove-section", "user"],
            RunOptions::default(),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation() {
        assert_eq!(identity_problem("Ana", "ana@example.com"), None);
        assert_eq!(identity_problem("  ", "a@b"), Some("Enter a name"));
        assert_eq!(identity_problem("Ana", ""), Some("Enter an email address"));
        assert_eq!(
            identity_problem("Ana", "ana"),
            Some("The email address needs an @")
        );
        assert_eq!(
            identity_problem("Ana", "ana @x.org"),
            Some("The email address cannot contain spaces")
        );
        assert_eq!(
            identity_problem("Ana", "<ana@x.org>"),
            Some("The email address cannot contain < or >")
        );
        assert_eq!(
            identity_problem("Ana <x>", "a@b"),
            Some("The name cannot contain < or >")
        );
        let identity = Identity::new(" Ana Lyst ", " ana@x.org ").unwrap();
        assert_eq!(identity.label(), "Ana Lyst <ana@x.org>");
        assert!(Identity::new("", "a@b").is_err());
    }

    #[test]
    fn resolved_identity() {
        let value = |value: &str, origin| {
            Some(ConfigValue {
                value: value.into(),
                origin,
            })
        };
        let none = ResolvedIdentity::default();
        assert_eq!(none.identity(), None);
        assert_eq!(none.origin(), None);
        let global = ResolvedIdentity {
            name: value("Ana", ConfigOrigin::Global),
            email: value("ana@x.org", ConfigOrigin::Global),
        };
        assert_eq!(global.identity().unwrap().label(), "Ana <ana@x.org>");
        assert!(!global.has_local());
        assert_eq!(global.origin(), Some(ConfigOrigin::Global));
        let mixed = ResolvedIdentity {
            email: value("work@x.org", ConfigOrigin::Local),
            ..global
        };
        assert!(mixed.has_local());
        assert_eq!(mixed.origin(), Some(ConfigOrigin::Local));
        let half = ResolvedIdentity {
            name: value("Ana", ConfigOrigin::System),
            email: None,
        };
        assert_eq!(half.identity(), None);
        assert_eq!(ConfigOrigin::System.description(), "from the system config");
    }
}
