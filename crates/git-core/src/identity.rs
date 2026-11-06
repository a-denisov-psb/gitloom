//! Product identity in one place (ADR-028).
//!
//! Every name-dependent value of the product lives here. Code elsewhere in
//! `crates/` must not spell the product name; `tests/identity_guard.rs` in
//! this crate enforces that. The values are final (ADR-033); a later rename
//! would only change this file plus the crate name in `Cargo.toml`.

/// Name shown to the user (window titles, texts, theme names).
pub const DISPLAY_NAME: &str = "Gitloom";

/// Lowercase name for the binary, temp file prefixes and reflog messages.
pub const BINARY_NAME: &str = "gitloom";

/// Application id (reverse DNS) for desktop metadata and packaging. Derived
/// from the GitHub account because no own domain exists.
pub const APP_ID: &str = "io.github.a_denisov_psb.Gitloom";

/// Name of the config/cache/data directory (the `application` part of
/// `directories::ProjectDirs`).
pub const DIR_NAME: &str = "Gitloom";

/// Public repository.
pub const REPOSITORY_URL: &str = "https://github.com/a-denisov-psb/gitloom";

/// Issue tracker.
pub const ISSUES_URL: &str = "https://github.com/a-denisov-psb/gitloom/issues";

/// Environment variable that tells a program started as `GIT_ASKPASS` /
/// `SSH_ASKPASS` that it runs in askpass mode. Internal between our own
/// processes, so it simply follows the product name.
pub const ASKPASS_ENV: &str = "GITLOOM_ASKPASS";

/// Environment variable naming the editor mode when started as Git editor.
pub const EDITOR_ENV: &str = "GITLOOM_EDITOR";

/// Environment variable naming the private directory that carries editor content.
pub const EDITOR_DIR_ENV: &str = "GITLOOM_EDITOR_DIR";

/// Debug builds only: names a directory; the app installs the crash report
/// hook for it and panics at once (integration test of crash reports).
pub const DEBUG_PANIC_ENV: &str = "GITLOOM_DEBUG_PANIC";

/// User agent for outgoing HTTP requests (avatars, update check).
pub fn user_agent(version: &str) -> String {
    format!("{DISPLAY_NAME}/{version}")
}

/// Prefix of a temporary file or directory, e.g. `gitloom-editor`.
pub fn temp_prefix(kind: &str) -> String {
    format!("{BINARY_NAME}-{kind}")
}

/// Reflog message of an undo/redo operation, e.g. `gitloom: undo`.
pub fn reflog_message(action: &str) -> String {
    format!("{BINARY_NAME}: {action}")
}

/// Title of a window of the product, optionally with a leading part.
pub fn window_title(part: Option<&str>) -> String {
    match part {
        Some(part) => format!("{part} – {DISPLAY_NAME}"),
        None => DISPLAY_NAME.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_compose_names() {
        assert_eq!(user_agent("1.2.3"), format!("{DISPLAY_NAME}/1.2.3"));
        assert_eq!(temp_prefix("editor"), format!("{BINARY_NAME}-editor"));
        assert_eq!(reflog_message("undo"), format!("{BINARY_NAME}: undo"));
        assert_eq!(window_title(None), DISPLAY_NAME);
        assert_eq!(window_title(Some("repo")), format!("repo – {DISPLAY_NAME}"));
    }
}
