//! Repositories inside WSL opened from Windows (M12-14, ADR-053): paths
//! under `\\wsl$\<distro>\…` or `\\wsl.localhost\<distro>\…`.

use std::path::Path;

/// Where a WSL path points: the distribution and the path inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WslLocation {
    pub distro: String,
    /// Absolute Linux path, e.g. `/home/me/project`.
    pub linux_path: String,
}

/// The WSL location of `path` on Windows; always `None` elsewhere.
pub fn wsl_location(path: &Path) -> Option<WslLocation> {
    if cfg!(windows) {
        parse_wsl_path(&path.to_string_lossy())
    } else {
        None
    }
}

/// Parses `\\wsl$\<distro>\…` and `\\wsl.localhost\<distro>\…` (any case,
/// `\` or `/`; also with the `\\?\UNC\` prefix of canonical paths).
pub fn parse_wsl_path(path: &str) -> Option<WslLocation> {
    let normalized = path.replace('\\', "/");
    let lower = normalized.to_ascii_lowercase();
    let rest = ["//?/unc/", "//"].iter().find_map(|prefix| {
        lower
            .strip_prefix(prefix)
            .map(|_| &normalized[prefix.len()..])
    })?;
    let (host, rest) = rest.split_once('/')?;
    if !host.eq_ignore_ascii_case("wsl$") && !host.eq_ignore_ascii_case("wsl.localhost") {
        return None;
    }
    let (distro, inner) = match rest.split_once('/') {
        Some((distro, inner)) => (distro, inner),
        None => (rest, ""),
    };
    if distro.is_empty() {
        return None;
    }
    let inner = inner.trim_end_matches('/');
    Some(WslLocation {
        distro: distro.to_owned(),
        linux_path: format!("/{inner}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(distro: &str, linux_path: &str) -> Option<WslLocation> {
        Some(WslLocation {
            distro: distro.into(),
            linux_path: linux_path.into(),
        })
    }

    #[test]
    fn wsl_prefixes_case_and_separators() {
        assert_eq!(
            parse_wsl_path(r"\\wsl$\Ubuntu\home\me\repo"),
            at("Ubuntu", "/home/me/repo")
        );
        assert_eq!(
            parse_wsl_path(r"\\wsl.localhost\Ubuntu-22.04\home\me\repo\"),
            at("Ubuntu-22.04", "/home/me/repo")
        );
        assert_eq!(
            parse_wsl_path("//WSL$/Debian/srv/x"),
            at("Debian", "/srv/x")
        );
        assert_eq!(
            parse_wsl_path(r"\\WSL.LOCALHOST\my.distro"),
            at("my.distro", "/")
        );
        assert_eq!(
            parse_wsl_path(r"\\?\UNC\wsl.localhost\Ubuntu\home\me"),
            at("Ubuntu", "/home/me")
        );
        // Other UNC shares and local paths are not WSL.
        assert_eq!(parse_wsl_path(r"\\server\share\repo"), None);
        assert_eq!(parse_wsl_path(r"\\wsl$\"), None);
        assert_eq!(parse_wsl_path(r"C:\Users\me\repo"), None);
        assert_eq!(parse_wsl_path("/home/me/repo"), None);
    }

    #[test]
    fn only_on_windows() {
        let path = Path::new(r"\\wsl$\Ubuntu\home\me\repo");
        assert_eq!(wsl_location(path).is_some(), cfg!(windows));
    }
}
