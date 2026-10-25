//! Reference name rules (`git check-ref-format --branch`), for instant
//! validation while typing. Git checks again when the branch is created.

/// Why `name` is not a valid branch name, if it is not.
pub fn check_branch_name(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        return Some("Enter a branch name");
    }
    if name == "@" || name.contains("@{") {
        return Some("Branch names cannot be \"@\" or contain \"@{\"");
    }
    if name.starts_with('-') {
        return Some("Branch names cannot start with \"-\"");
    }
    if name
        .chars()
        .any(|c| c.is_ascii_control() || " ~^:?*[\\".contains(c))
    {
        return Some("Branch names cannot contain spaces or ~ ^ : ? * [ \\");
    }
    if name.contains("..") {
        return Some("Branch names cannot contain \"..\"");
    }
    if name.starts_with('/') || name.ends_with('/') || name.contains("//") {
        return Some("Branch names cannot start or end with \"/\" or contain \"//\"");
    }
    if name.ends_with('.') {
        return Some("Branch names cannot end with \".\"");
    }
    if name
        .split('/')
        .any(|part| part.starts_with('.') || part.ends_with(".lock"))
    {
        return Some("No part of a branch name may start with \".\" or end with \".lock\"");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_names() {
        for name in [
            "main",
            "feature/M3-07-create-branch",
            "fix_1",
            "release/0.4.0",
            "ümlaut",
        ] {
            assert_eq!(check_branch_name(name), None, "{name}");
        }
    }

    #[test]
    fn invalid_names() {
        for name in [
            "",
            "@",
            "a@{b",
            "-x",
            "with space",
            "a~b",
            "a^b",
            "a:b",
            "a?b",
            "a*b",
            "a[b",
            "a\\b",
            "a..b",
            "/a",
            "a/",
            "a//b",
            "a.",
            ".a",
            "a/.b",
            "a.lock",
            "a/b.lock",
            "tab\tx",
        ] {
            assert!(check_branch_name(name).is_some(), "{name:?}");
        }
    }
}
