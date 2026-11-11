//! Signing settings in the global config, repository overrides and an SSH
//! signed commit (M10-04, ADR-046). One test: it points
//! `GIT_CONFIG_GLOBAL` at a temporary file for the whole process.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::signing::{
    global_signing, local_signing_overrides, set_global_signing, SigningFormat,
};
use support::TestRepo;

#[test]
fn signing_settings_round_trip_and_ssh_signing() {
    let dir = tempfile::tempdir().unwrap();
    let global = dir.path().join("gitconfig");
    std::fs::write(&global, "").unwrap();
    std::env::set_var("GIT_CONFIG_GLOBAL", &global);

    let empty = global_signing().unwrap();
    assert_eq!(empty.format, SigningFormat::Gpg);
    assert!(!empty.sign_commits && empty.key.is_none());

    set_global_signing("gpg.format", Some("ssh")).unwrap();
    set_global_signing("user.signingkey", Some("/keys/id.pub")).unwrap();
    set_global_signing("commit.gpgSign", Some("true")).unwrap();
    set_global_signing("tag.gpgSign", Some("true")).unwrap();
    let written = global_signing().unwrap();
    assert_eq!(written.format, SigningFormat::Ssh);
    assert_eq!(written.key.as_deref(), Some("/keys/id.pub"));
    assert!(written.sign_commits && written.sign_tags);
    set_global_signing("tag.gpgSign", None).unwrap();
    assert!(!global_signing().unwrap().sign_tags);
    assert!(set_global_signing("user.name", Some("x")).is_err());

    // An unsigned commit has no badge.
    let unsigned = TestRepo::new();
    unsigned.write("u.txt", "u\n");
    unsigned.git(&["add", "u.txt"]);
    unsigned.git(&[
        "-c",
        "commit.gpgSign=false",
        "commit",
        "--quiet",
        "-m",
        "Unsigned",
    ]);
    let cli = git_core::CliBackend::new(unsigned.path());
    assert_eq!(
        git_core::signature::signature(&cli, &unsigned.head()).unwrap(),
        None
    );

    // A repository value overrides the global one and is reported.
    let repo = TestRepo::new();
    repo.git(&["config", "commit.gpgSign", "false"]);
    assert_eq!(
        local_signing_overrides(repo.path()).unwrap(),
        [("commit.gpgSign".to_owned(), "false".to_owned())]
    );

    // SSH signing, where ssh-keygen is available (Unix: Windows OpenSSH
    // refuses keys whose file permissions it considers too open).
    if !cfg!(unix) {
        return;
    }
    let keygen = std::process::Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", "test", "-f"])
        .arg(dir.path().join("id"))
        .status();
    if !keygen.is_ok_and(|s| s.success()) {
        eprintln!("ssh-keygen not available: SSH signing not tested");
        return;
    }
    let key = dir.path().join("id.pub");
    set_global_signing("user.signingkey", Some(key.to_str().unwrap())).unwrap();
    repo.git(&["config", "--unset", "commit.gpgSign"]);
    repo.write("a.txt", "a\n");
    repo.git(&["add", "a.txt"]);
    // The test helper sets HOME but keeps GIT_CONFIG_GLOBAL from the env.
    repo.git(&["commit", "--quiet", "-m", "Signed"]);
    // Without `gpg.ssh.allowedSignersFile` Git cannot verify SSH signatures
    // (`%G?` reads N then); the signature header shows it was signed.
    let commit = repo.git(&["cat-file", "commit", "HEAD"]);
    assert!(
        commit.contains("gpgsig -----BEGIN SSH SIGNATURE-----"),
        "{commit}"
    );
    // The details badge reads it as signed with the key not available (M10-05).
    let cli = git_core::CliBackend::new(repo.path());
    let head = repo.head();
    let status = git_core::signature::signature(&cli, &head)
        .unwrap()
        .unwrap();
    assert_eq!(
        status.state,
        git_core::signature::SignatureState::KeyMissing
    );
    // An allowed signers file makes it verifiable.
    let allowed = dir.path().join("allowed_signers");
    let public = std::fs::read_to_string(&key).unwrap();
    std::fs::write(&allowed, format!("author@example.com {public}")).unwrap();
    repo.git(&[
        "config",
        "gpg.ssh.allowedSignersFile",
        allowed.to_str().unwrap(),
    ]);
    let status = git_core::signature::signature(&cli, &head)
        .unwrap()
        .unwrap();
    assert!(
        matches!(
            status.state,
            git_core::signature::SignatureState::Good
                | git_core::signature::SignatureState::Untrusted
        ),
        "{status:?}"
    );
}
