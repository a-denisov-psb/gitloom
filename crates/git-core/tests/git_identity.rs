//! Git identity of a repository (M8-02). Its own test binary with one test:
//! `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_NOSYSTEM` point to temporary values
//! for the whole process, so the user's own config is never touched.

// Integration tests are test-only code; helpers outside `#[test]` may panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use git_core::{
    global_identity, set_identity, unset_local_identity, write_global_identity, CliBackend,
    ConfigOrigin, GitBackend, GitWriter, GixBackend, Identity, IdentityScope,
};
use support::TestRepo;

fn identity(name: &str, email: &str) -> Identity {
    Identity::new(name, email).unwrap()
}

#[test]
fn identity_resolves_local_over_global_and_writes_cleanly() {
    let repo = TestRepo::new();
    repo.commit("a.txt", "a\n", "Base");
    let global = repo.path().join("..").join("global.gitconfig");
    std::fs::write(&global, "").unwrap();
    std::env::set_var("GIT_CONFIG_GLOBAL", &global);
    std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
    let backend = GixBackend::discover(repo.path()).unwrap();
    let cli = CliBackend::new(repo.path());

    // Nothing set: no identity.
    let resolved = backend.identity().unwrap();
    assert_eq!(resolved.identity(), None);
    assert_eq!(resolved.origin(), None);
    assert_eq!(global_identity().unwrap(), None);

    // Global read and write outside any repository (M8-03).
    write_global_identity(&identity("First Name", "first@x.org")).unwrap();
    assert_eq!(
        global_identity().unwrap(),
        Some(identity("First Name", "first@x.org"))
    );

    // Global write.
    set_identity(
        &cli,
        IdentityScope::Global,
        &identity("Ana Lyst", "ana@x.org"),
    )
    .unwrap();
    let text = std::fs::read_to_string(&global).unwrap();
    assert!(
        text.contains("name = Ana Lyst") && text.contains("email = ana@x.org"),
        "{text}"
    );
    let resolved = backend.identity().unwrap();
    assert_eq!(resolved.identity(), Some(identity("Ana Lyst", "ana@x.org")));
    assert_eq!(resolved.origin(), Some(ConfigOrigin::Global));

    // Local over global.
    set_identity(
        &cli,
        IdentityScope::Local,
        &identity("Ana Work", "ana@work.org"),
    )
    .unwrap();
    let resolved = backend.identity().unwrap();
    assert_eq!(
        resolved.identity(),
        Some(identity("Ana Work", "ana@work.org"))
    );
    assert_eq!(resolved.name.as_ref().unwrap().origin, ConfigOrigin::Local);
    assert!(resolved.has_local());
    assert_eq!(resolved.inherited, Some(identity("Ana Lyst", "ana@x.org")));
    repo.git(&["config", "--local", "--unset", "user.name"]);
    let mixed = backend.identity().unwrap();
    assert_eq!(mixed.identity(), Some(identity("Ana Lyst", "ana@work.org")));
    assert_eq!(mixed.origin(), Some(ConfigOrigin::Local));

    // The next commit uses it as author and committer (M8-04).
    std::fs::write(repo.path().join("b.txt"), "b\n").unwrap();
    repo.git(&["add", "b.txt"]);
    cli.commit("With the repository identity", false).unwrap();
    // (name from the global config, email from the repository config)
    assert_eq!(
        repo.git(&["log", "-1", "--format=%an <%ae> / %cn <%ce>"]),
        "Ana Lyst <ana@work.org> / Ana Lyst <ana@work.org>"
    );

    // Unset local: back to global, no empty [user] section, and a second
    // unset (keys missing) is fine.
    unset_local_identity(&cli).unwrap();
    unset_local_identity(&cli).unwrap();
    let local = std::fs::read_to_string(repo.path().join(".git").join("config")).unwrap();
    assert!(!local.contains("[user]"), "{local}");
    let resolved = backend.identity().unwrap();
    assert_eq!(resolved.identity(), Some(identity("Ana Lyst", "ana@x.org")));
    assert!(!resolved.has_local());

    // A change made outside Gitloom is seen by the open backend.
    repo.git(&["config", "--local", "user.name", "Terminal"]);
    repo.git(&["config", "--local", "user.email", "t@x.org"]);
    assert_eq!(
        backend.identity().unwrap().identity(),
        Some(identity("Terminal", "t@x.org"))
    );
    std::fs::write(repo.path().join("c.txt"), "c\n").unwrap();
    repo.git(&["add", "c.txt"]);
    cli.commit("With a chosen identity", false).unwrap();
    assert_eq!(
        repo.git(&["log", "-1", "--format=%an <%ae> / %cn <%ce>"]),
        "Terminal <t@x.org> / Terminal <t@x.org>"
    );
    // Other keys of the section stay when the identity is unset.
    repo.git(&["config", "--local", "user.signingkey", "ABC"]);
    unset_local_identity(&cli).unwrap();
    let local = std::fs::read_to_string(repo.path().join(".git").join("config")).unwrap();
    assert!(local.contains("signingkey = ABC"), "{local}");

    // An empty value counts as unset; invalid identities are refused.
    std::fs::write(&global, "[user]\n\tname =\n\temail = e@x.org\n").unwrap();
    assert_eq!(backend.identity().unwrap().name, None);
    let invalid = Identity {
        name: "Ana".into(),
        email: "no-at".into(),
    };
    assert!(set_identity(&cli, IdentityScope::Local, &invalid).is_err());
}
