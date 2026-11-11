//! Commit and tag signing settings (M10-04, ADR-046): read and written in
//! the global Git config; repository values are shown as overrides. GPG
//! secret keys are listed with the GPG program's colon format.
//!
//! Blocking: runs `git` and `gpg`.

use std::path::Path;

use crate::{global_config, set_global_config, CliBackend, Error, GitWriter, Result, RunOptions};

/// `gpg.format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SigningFormat {
    /// OpenPGP (Git's default).
    #[default]
    Gpg,
    Ssh,
}

impl SigningFormat {
    /// Value written to `gpg.format`.
    pub fn config_value(self) -> &'static str {
        match self {
            Self::Gpg => "openpgp",
            Self::Ssh => "ssh",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Gpg => "GPG (OpenPGP)",
            Self::Ssh => "SSH",
        }
    }

    fn from_config(value: Option<&str>) -> Self {
        match value.map(str::to_ascii_lowercase).as_deref() {
            Some("ssh") => Self::Ssh,
            _ => Self::Gpg,
        }
    }
}

/// The Git config keys of the signing settings, in display order.
pub const SIGNING_KEYS: [&str; 5] = [
    "gpg.format",
    "user.signingkey",
    "gpg.program",
    "commit.gpgSign",
    "tag.gpgSign",
];

/// Signing settings of one config scope.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SigningSettings {
    pub format: SigningFormat,
    /// `user.signingkey`: GPG key id, or the path of an SSH `.pub` file.
    pub key: Option<String>,
    /// `gpg.program`; `None` = Git's default (`gpg`).
    pub program: Option<String>,
    pub sign_commits: bool,
    pub sign_tags: bool,
}

fn is_true(value: Option<&str>) -> bool {
    matches!(
        value.map(str::to_ascii_lowercase).as_deref(),
        Some("true" | "yes" | "on" | "1")
    )
}

impl SigningSettings {
    fn from_values(get: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            format: SigningFormat::from_config(get("gpg.format").as_deref()),
            key: get("user.signingkey").filter(|v| !v.is_empty()),
            program: get("gpg.program").filter(|v| !v.is_empty()),
            sign_commits: is_true(get("commit.gpgSign").as_deref()),
            sign_tags: is_true(get("tag.gpgSign").as_deref()),
        }
    }
}

/// The signing settings of the global Git config.
pub fn global_signing() -> Result<SigningSettings> {
    let mut values = Vec::new();
    for key in SIGNING_KEYS {
        values.push((key, global_config(key)?));
    }
    Ok(SigningSettings::from_values(|key| {
        values
            .iter()
            .find(|(k, _)| *k == key)
            .and_then(|(_, v)| v.clone())
    }))
}

/// Writes one signing key of the global config; `None` removes it.
pub fn set_global_signing(key: &str, value: Option<&str>) -> Result<()> {
    if !SIGNING_KEYS.contains(&key) {
        return Err(Error::Backend(format!("{key} is not a signing setting")));
    }
    set_global_config(key, value)
}

/// Signing keys set in the repository's own config (`--local`), with their
/// values: they override the global settings (shown read-only).
pub fn local_signing_overrides(root: &Path) -> Result<Vec<(String, String)>> {
    let git = CliBackend::new(root);
    let read = RunOptions {
        read_only: true,
        ..RunOptions::default()
    };
    let mut overrides = Vec::new();
    for key in SIGNING_KEYS {
        let (code, output) = git.run_status(&["config", "--local", "--get", key], read)?;
        if code == Some(0) {
            overrides.push((key.to_owned(), output.stdout_text().trim().to_owned()));
        }
    }
    Ok(overrides)
}

/// A GPG secret key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpgKey {
    /// Long key id (16 hex digits).
    pub id: String,
    /// First user id, e.g. "Ana Lyst <ana@example.com>".
    pub user: String,
    /// Expiry as Unix time, if the key expires.
    pub expires: Option<i64>,
    /// Expired or revoked (GPG's validity field, or the expiry before `now`).
    pub unusable: bool,
}

impl GpgKey {
    /// "ID · user", with "(expired)" for unusable keys.
    pub fn label(&self) -> String {
        if self.unusable {
            format!("{} · {} (expired)", self.id, self.user)
        } else {
            format!("{} · {}", self.id, self.user)
        }
    }
}

/// Parses `gpg --list-secret-keys --with-colons` output: one key per `sec`
/// record, its first `uid` as user. `now` is the current Unix time.
pub fn parse_secret_keys(output: &str, now: i64) -> Vec<GpgKey> {
    let mut keys: Vec<GpgKey> = Vec::new();
    for line in output.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        match fields.first().copied() {
            Some("sec") => {
                let validity = fields.get(1).copied().unwrap_or("");
                let id = fields.get(4).copied().unwrap_or("").to_owned();
                let expires = fields
                    .get(6)
                    .and_then(|v| v.parse::<i64>().ok())
                    .filter(|&t| t > 0);
                let unusable =
                    matches!(validity, "e" | "r" | "d") || expires.is_some_and(|t| t <= now);
                if !id.is_empty() {
                    keys.push(GpgKey {
                        id,
                        user: String::new(),
                        expires,
                        unusable,
                    });
                }
            }
            Some("uid") => {
                if let Some(key) = keys.last_mut().filter(|k| k.user.is_empty()) {
                    key.user = fields.get(9).copied().unwrap_or("").replace("\\x3a", ":");
                }
            }
            _ => {}
        }
    }
    keys
}

/// Secret keys of the GPG program (`program`, else `gpg`).
pub fn list_gpg_keys(program: Option<&str>) -> Result<Vec<GpgKey>> {
    let program = program.filter(|p| !p.trim().is_empty()).unwrap_or("gpg");
    let output = crate::cli_backend::command(Path::new(program))
        .args(["--list-secret-keys", "--with-colons", "--keyid-format=long"])
        .output()
        .map_err(|e| Error::Backend(format!("GPG was not found ({program}): {e}")))?;
    if !output.status.success() {
        return Err(Error::Command {
            command: format!("{program} --list-secret-keys"),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    Ok(parse_secret_keys(
        &String::from_utf8_lossy(&output.stdout),
        now,
    ))
}

/// A Git error caused by signing (missing key, GPG or agent failed).
pub fn signing_failed(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    error.contains("gpg failed to sign")
        || error.contains("failed to sign the data")
        || error.contains("signing failed")
        || error.contains("ssh-keygen")
        || error.contains("no secret key")
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLONS: &str = "\
sec:u:4096:1:AAAABBBBCCCCDDDD:1600000000:::u:::scESC:::+:::23::0:
fpr:::::::::0123456789ABCDEF0123AAAABBBBCCCCDDDD:
grp:::::::::0123456789:
uid:u::::1600000000::HASH::Ana Lyst <ana@example.com>::::::::::0:
uid:u::::1600000000::HASH::Ana Second <ana@second.example>::::::::::0:
ssb:u:4096:1:1111222233334444:1600000000::::::e:::+:::23:
sec:e:2048:1:EEEEFFFF00001111:1500000000:1550000000::u:::scESC:::+:::23::0:
uid:e::::1500000000::HASH::Old Key\\x3a work <old@example.com>::::::::::0:
sec:u:255:22:9999888877776666:1700000000:1800000000::u:::scESC:::+:::ed25519:::0:
uid:u::::1700000000::HASH::Expires Later <later@example.com>::::::::::0:
";

    #[test]
    fn colon_output_is_parsed() {
        let keys = parse_secret_keys(COLONS, 1_750_000_000);
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0].id, "AAAABBBBCCCCDDDD");
        assert_eq!(keys[0].user, "Ana Lyst <ana@example.com>", "first uid");
        assert!(!keys[0].unusable);
        assert_eq!(keys[0].expires, None);
        assert!(keys[1].unusable, "validity e");
        assert_eq!(keys[1].user, "Old Key: work <old@example.com>");
        assert_eq!(keys[2].expires, Some(1_800_000_000));
        assert!(!keys[2].unusable);
        assert!(parse_secret_keys(COLONS, 1_900_000_000)[2].unusable);
        assert_eq!(
            keys[1].label(),
            "EEEEFFFF00001111 · Old Key: work <old@example.com> (expired)"
        );
    }

    #[test]
    fn no_keys() {
        assert!(parse_secret_keys("", 0).is_empty());
        assert!(parse_secret_keys("tru::1:1700000000:0:3:1:5\n", 0).is_empty());
    }

    #[test]
    fn missing_gpg_is_an_error() {
        let error = list_gpg_keys(Some("/nonexistent/gpg-program")).unwrap_err();
        assert!(error.to_string().contains("GPG was not found"), "{error}");
    }

    #[test]
    fn settings_from_values() {
        let settings = SigningSettings::from_values(|key| match key {
            "gpg.format" => Some("ssh".into()),
            "user.signingkey" => Some("/home/ana/.ssh/id_ed25519.pub".into()),
            "commit.gpgSign" => Some("true".into()),
            "tag.gpgSign" => Some("false".into()),
            _ => None,
        });
        assert_eq!(settings.format, SigningFormat::Ssh);
        assert!(settings.sign_commits && !settings.sign_tags);
        assert_eq!(settings.program, None);
        assert_eq!(
            SigningSettings::from_values(|_| None).format,
            SigningFormat::Gpg
        );
    }

    #[test]
    fn signing_errors_are_recognized() {
        assert!(signing_failed(
            "error: gpg failed to sign the data\nfatal: failed to write commit object"
        ));
        assert!(signing_failed("error: Load key \"x\": ... ssh-keygen ..."));
        assert!(!signing_failed("nothing to commit"));
    }
}
