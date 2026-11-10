//! SSH key per repository through `core.sshCommand` (M8-05, ADR-041): the
//! setting with its origin, the command Gitloom writes, key discovery and
//! the writes. Keys are referenced by path, never copied; passphrases go
//! through the askpass prompt and are not stored.

use std::path::{Path, PathBuf};

use crate::{ConfigOrigin, Error, GitWriter, Result, RunOptions};

/// What `core.sshCommand` says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshSetting {
    /// Not set: ssh-agent and `~/.ssh/config` decide.
    SystemDefault,
    /// The form Gitloom writes: `ssh -i "<key>" -o IdentitiesOnly=yes`.
    Key(PathBuf),
    /// Any other command (shown read-only, replaced only after
    /// confirmation).
    Custom(String),
}

/// `core.sshCommand` of a repository with the file it comes from (`None`
/// for the system default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshConfig {
    pub setting: SshSetting,
    pub origin: Option<ConfigOrigin>,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            setting: SshSetting::SystemDefault,
            origin: None,
        }
    }
}

impl SshConfig {
    /// Reads the setting from the value of `core.sshCommand`.
    pub fn from_value(value: Option<(&str, ConfigOrigin)>) -> Self {
        match value {
            Some((command, origin)) if !command.trim().is_empty() => Self {
                setting: parse_ssh_command(command),
                origin: Some(origin),
            },
            _ => Self::default(),
        }
    }
}

const PREFIX: &str = "ssh -i \"";
const SUFFIX: &str = "\" -o IdentitiesOnly=yes";

/// The command Gitloom writes for `key`: forward slashes (also on Windows,
/// where Git runs it through its shell), the path in double quotes. A path
/// containing `"` cannot be quoted this way and is refused.
pub fn ssh_command(key: &Path) -> Result<String> {
    let path = key.to_string_lossy().replace('\\', "/");
    if path.contains('"') {
        return Err(Error::Backend(
            "The key path contains a double quote (\"), which cannot be used in core.sshCommand"
                .into(),
        ));
    }
    if path.trim().is_empty() {
        return Err(Error::Backend("Choose a key file".into()));
    }
    Ok(format!("{PREFIX}{path}{SUFFIX}"))
}

/// The setting a `core.sshCommand` value stands for: the form of
/// [`ssh_command`] is a key, anything else a custom command.
pub fn parse_ssh_command(command: &str) -> SshSetting {
    let trimmed = command.trim();
    match trimmed
        .strip_prefix(PREFIX)
        .and_then(|rest| rest.strip_suffix(SUFFIX))
    {
        Some(path) if !path.is_empty() && !path.contains('"') => {
            SshSetting::Key(PathBuf::from(path))
        }
        _ => SshSetting::Custom(trimmed.to_owned()),
    }
}

/// `path` with forward slashes, to compare key paths the way they are
/// written to `core.sshCommand`.
pub fn normalized_key_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Private keys in `dir` (usually `~/.ssh`): files `X` with a sibling
/// `X.pub`; `known_hosts*`, `config` and `authorized_keys` are never keys.
/// Sorted by file name; a missing directory has none.
pub fn find_keys(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut keys: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                return false;
            };
            let excluded = name.starts_with("known_hosts")
                || name == "config"
                || name == "authorized_keys"
                || name.ends_with(".pub");
            !excluded && dir.join(format!("{name}.pub")).is_file()
        })
        .collect();
    keys.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    keys
}

/// Uses `key` for the repository's network operations (repository config).
pub fn set_ssh_key(writer: &dyn GitWriter, key: &Path) -> Result<()> {
    let command = ssh_command(key)?;
    writer
        .run(
            &["config", "--local", "core.sshCommand", &command],
            RunOptions::default(),
        )
        .map(drop)
}

/// Back to the system default: removes `core.sshCommand` from the
/// repository config (a missing key is not an error). Other `core` keys
/// always remain, so no empty section is left.
pub fn unset_ssh_command(writer: &dyn GitWriter) -> Result<()> {
    let (code, output) = writer.run_status(
        &["config", "--local", "--unset-all", "core.sshCommand"],
        RunOptions::default(),
    )?;
    // 5: the key does not exist.
    if matches!(code, Some(0) | Some(5)) {
        Ok(())
    } else {
        Err(Error::Command {
            command: "git config --local --unset-all core.sshCommand".into(),
            code,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// `url` is reached over SSH: `ssh://`, `git+ssh://`, `ssh+git://` or the
/// scp-like `[user@]host:path` (not a Windows drive like `C:\x`).
pub fn is_ssh_url(url: &str) -> bool {
    let url = url.trim();
    if ["ssh://", "git+ssh://", "ssh+git://"]
        .iter()
        .any(|scheme| url.starts_with(scheme))
    {
        return true;
    }
    if url.contains("://") {
        return false;
    }
    let Some((host, _path)) = url.split_once(':') else {
        return false;
    };
    // `C:` or `C:\x`: a local Windows path; a slash before the colon: a
    // local path containing a colon.
    let drive = host.len() == 1 && host.chars().all(|c| c.is_ascii_alphabetic());
    !host.is_empty() && !drive && !host.contains('/') && !host.contains('\\')
}

/// Any of the remote URLs uses SSH.
pub fn has_ssh_remotes<'a>(urls: impl IntoIterator<Item = &'a str>) -> bool {
    urls.into_iter().any(is_ssh_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_round_trip() {
        let unix = ssh_command(Path::new("/home/me/.ssh/id_work")).unwrap();
        assert_eq!(
            unix,
            r#"ssh -i "/home/me/.ssh/id_work" -o IdentitiesOnly=yes"#
        );
        assert_eq!(
            parse_ssh_command(&unix),
            SshSetting::Key(PathBuf::from("/home/me/.ssh/id_work"))
        );
        // Windows paths get forward slashes; spaces stay inside the quotes.
        let windows = ssh_command(Path::new(r"C:\Users\Ana Lyst\.ssh\id_ed25519")).unwrap();
        assert_eq!(
            windows,
            r#"ssh -i "C:/Users/Ana Lyst/.ssh/id_ed25519" -o IdentitiesOnly=yes"#
        );
        assert_eq!(
            parse_ssh_command(&windows),
            SshSetting::Key(PathBuf::from("C:/Users/Ana Lyst/.ssh/id_ed25519"))
        );
        assert!(ssh_command(Path::new(r#"/tmp/a"b"#)).is_err());
        assert!(ssh_command(Path::new("")).is_err());
    }

    #[test]
    fn other_commands_are_custom() {
        for command in [
            "ssh -i ~/.ssh/id_rsa",
            r#"ssh -i "/k" -o IdentitiesOnly=yes -v"#,
            "plink -batch",
            r#"ssh -i "" -o IdentitiesOnly=yes"#,
        ] {
            assert_eq!(
                parse_ssh_command(command),
                SshSetting::Custom(command.to_owned()),
                "{command}"
            );
        }
        let config = SshConfig::from_value(Some(("ssh -v", ConfigOrigin::Global)));
        assert_eq!(config.setting, SshSetting::Custom("ssh -v".into()));
        assert_eq!(config.origin, Some(ConfigOrigin::Global));
        assert_eq!(
            SshConfig::from_value(Some(("  ", ConfigOrigin::Local))),
            SshConfig::default()
        );
        assert_eq!(
            SshConfig::from_value(None).setting,
            SshSetting::SystemDefault
        );
    }

    #[test]
    fn keys_are_files_with_a_public_sibling() {
        let dir = std::env::temp_dir().join(format!("gitloom-ssh-keys-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for name in [
            "id_rsa",
            "id_rsa.pub",
            "id_work",
            "id_work.pub",
            "lonely",
            "known_hosts",
            "known_hosts.pub",
            "known_hosts.old",
            "config",
            "config.pub",
            "authorized_keys",
            "authorized_keys.pub",
            "only.pub",
        ] {
            std::fs::write(dir.join(name), "x").unwrap();
        }
        std::fs::create_dir_all(dir.join("folder")).unwrap();
        std::fs::write(dir.join("folder.pub"), "x").unwrap();
        let names: Vec<String> = find_keys(&dir)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["id_rsa", "id_work"]);
        assert!(find_keys(&dir.join("missing")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ssh_urls() {
        for url in [
            "ssh://git@github.com/a/b.git",
            "git+ssh://host/repo",
            "git@github-fc:a-denisov-psb/gitloom.git",
            "host:repo.git",
        ] {
            assert!(is_ssh_url(url), "{url}");
        }
        for url in [
            "https://github.com/a/b.git",
            "file:///srv/repo",
            "/srv/repo",
            r"C:\repos\x",
            "C:/repos/x",
            "./relative:with-colon",
            "",
        ] {
            assert!(!is_ssh_url(url), "{url}");
        }
        assert!(has_ssh_remotes(["https://x/y", "git@h:p"]));
        assert!(!has_ssh_remotes(["https://x/y"]));
        assert_eq!(normalized_key_path(Path::new(r"C:\k\id")), "C:/k/id");
    }
}
