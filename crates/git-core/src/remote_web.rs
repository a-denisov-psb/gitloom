//! Website of a remote (M12-03): the repository start page derived offline
//! from the remote URL, and the menu label for well-known hosts.

/// `https://host/owner/repo` for scp-like (`git@host:owner/repo.git`),
/// `ssh://[user@]host[:port]/…` and `http(s)://[user[:pw]@]host[:port]/…`
/// URLs: credentials, SSH ports and `.git` removed, HTTPS ports kept, host
/// lowercased. `None` for local paths, `file://` and anything else.
pub fn web_url(remote: &str) -> Option<String> {
    let remote = remote.trim();
    let (host, path, port) = if let Some(rest) = remote
        .strip_prefix("https://")
        .or_else(|| remote.strip_prefix("http://"))
    {
        let (authority, path) = rest.split_once('/')?;
        let authority = authority.rsplit('@').next()?;
        let (host, port) = match authority.split_once(':') {
            Some((host, port)) if !port.is_empty() => (host, Some(port)),
            Some((host, _)) => (host, None),
            None => (authority, None),
        };
        (host, path, port)
    } else if let Some(rest) = remote.strip_prefix("ssh://") {
        let (authority, path) = rest.split_once('/')?;
        let authority = authority.rsplit('@').next()?;
        // The SSH port has nothing to do with the website.
        let host = authority.split(':').next()?;
        (host, path, None)
    } else if remote.contains("://") {
        return None;
    } else {
        // scp-like: [user@]host:path; a drive letter or a path is local.
        let (authority, path) = remote.split_once(':')?;
        if authority.len() <= 1 || authority.contains('/') || authority.contains('\\') {
            return None;
        }
        let host = authority.rsplit('@').next()?;
        (host, path, None)
    };
    let host = host.trim().to_lowercase();
    let path = path
        .trim_matches('/')
        .trim_end_matches(".git")
        .trim_end_matches('/');
    if host.is_empty() || path.is_empty() || !host.contains('.') && host != "localhost" {
        return None;
    }
    Some(match port {
        Some(port) => format!("https://{host}:{port}/{path}"),
        None => format!("https://{host}/{path}"),
    })
}

/// Menu label for a website from [`web_url`].
pub fn web_label(url: &str) -> &'static str {
    let host = url
        .trim_start_matches("https://")
        .split(['/', ':'])
        .next()
        .unwrap_or_default();
    match host {
        "github.com" => "Open on GitHub",
        "gitlab.com" => "Open on GitLab",
        "bitbucket.org" => "Open on Bitbucket",
        _ => "Open in browser",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_forms() {
        let gh = Some("https://github.com/owner/repo".to_owned());
        assert_eq!(web_url("git@github.com:owner/repo.git"), gh);
        assert_eq!(web_url("github.com:owner/repo"), gh);
        assert_eq!(web_url("ssh://git@github.com/owner/repo.git"), gh);
        assert_eq!(web_url("ssh://git@github.com:22/owner/repo.git"), gh);
        assert_eq!(web_url("https://github.com/owner/repo.git"), gh);
        assert_eq!(web_url("https://github.com/owner/repo/"), gh);
        assert_eq!(web_url("https://user:secret@github.com/owner/repo.git"), gh);
        assert_eq!(web_url("https://user@GitHub.COM/owner/repo"), gh);
        assert_eq!(web_url("http://github.com/owner/repo.git"), gh);
        // HTTPS ports stay.
        assert_eq!(
            web_url("https://git.example.org:8443/group/sub/repo.git"),
            Some("https://git.example.org:8443/group/sub/repo".to_owned())
        );
        // Local paths and file URLs have no website.
        for local in [
            "/srv/git/repo.git",
            "../repo",
            "C:\\repos\\x",
            "C:/repos/x",
            "file:///srv/git/repo.git",
            "",
        ] {
            assert_eq!(web_url(local), None, "{local}");
        }
    }

    #[test]
    fn labels_per_host() {
        assert_eq!(web_label("https://github.com/o/r"), "Open on GitHub");
        assert_eq!(web_label("https://gitlab.com/o/r"), "Open on GitLab");
        assert_eq!(web_label("https://bitbucket.org/o/r"), "Open on Bitbucket");
        assert_eq!(
            web_label("https://git.example.org:8443/o/r"),
            "Open in browser"
        );
    }
}
