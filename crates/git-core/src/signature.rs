//! Signature status of a commit for the commit details (M10-05):
//! `git log -1 --format=%G?%x00%GS%x00%GK <commit>`.
//!
//! Blocking: runs `git`.

use crate::{GitWriter, Result, RunOptions};

/// What Git says about a commit's signature (`%G?`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureState {
    /// G: good signature.
    Good,
    /// U: good signature, trust of the key unknown.
    Untrusted,
    /// X / Y: the signature or its key has expired.
    Expired,
    /// R: the key was revoked.
    Revoked,
    /// E: cannot be checked (key not available; for SSH also without
    /// `gpg.ssh.allowedSignersFile`).
    KeyMissing,
    /// B: bad signature.
    Bad,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub state: SignatureState,
    /// Signer (`%GS`), may be empty.
    pub signer: String,
    /// Key id or fingerprint (`%GK`), for the tooltip.
    pub key: String,
}

impl Signature {
    /// Badge text in the commit details.
    pub fn text(&self) -> String {
        let by = |extra: &str| {
            if self.signer.is_empty() {
                format!("Signed{extra}")
            } else {
                format!("Signed by {}{extra}", self.signer)
            }
        };
        match self.state {
            SignatureState::Good => by(""),
            SignatureState::Untrusted => by(" (unverified trust)"),
            SignatureState::Expired => "Signed, key expired".into(),
            SignatureState::Revoked => "Signed, key revoked".into(),
            SignatureState::KeyMissing => "Signed (key not available)".into(),
            SignatureState::Bad => "Bad signature".into(),
        }
    }
}

/// Parses `%G?%x00%GS%x00%GK`; `None` for an unsigned commit (N) or
/// unknown output.
pub fn parse_signature(output: &str) -> Option<Signature> {
    let mut fields = output.trim_end_matches(['\n', '\r']).split('\0');
    let state = match fields.next()?.trim() {
        "G" => SignatureState::Good,
        "U" => SignatureState::Untrusted,
        "X" | "Y" => SignatureState::Expired,
        "R" => SignatureState::Revoked,
        "E" => SignatureState::KeyMissing,
        "B" => SignatureState::Bad,
        _ => return None,
    };
    Some(Signature {
        state,
        signer: fields.next().unwrap_or("").trim().to_owned(),
        key: fields.next().unwrap_or("").trim().to_owned(),
    })
}

/// Signature status of commit `id`. A commit Git reports as unsigned
/// (N) that carries a signature header could not be verified (SSH without
/// an allowed signers file): [`SignatureState::KeyMissing`].
pub fn signature(git: &dyn GitWriter, id: &str) -> Result<Option<Signature>> {
    let read = RunOptions {
        read_only: true,
        ..RunOptions::default()
    };
    let output = git.run(&["log", "-1", "--format=%G?%x00%GS%x00%GK", id, "--"], read)?;
    let text = output.stdout_text();
    if let Some(signature) = parse_signature(&text) {
        return Ok(Some(signature));
    }
    let commit = git.run(&["cat-file", "commit", id], read)?.stdout_text();
    let header = commit.split("\n\n").next().unwrap_or("");
    let signed = header
        .lines()
        .any(|line| line.starts_with("gpgsig ") || line.starts_with("gpgsig-sha256 "));
    Ok(signed.then(|| Signature {
        state: SignatureState::KeyMissing,
        signer: String::new(),
        key: String::new(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_value() {
        let parse = |s: &str| parse_signature(s).map(|sig| (sig.state, sig.text()));
        assert_eq!(
            parse("G\0Ana <ana@example.com>\0ABCD\n"),
            Some((
                SignatureState::Good,
                "Signed by Ana <ana@example.com>".into()
            ))
        );
        assert_eq!(
            parse("U\0Ana\0ABCD"),
            Some((
                SignatureState::Untrusted,
                "Signed by Ana (unverified trust)".into()
            ))
        );
        for expired in ["X\0a\0k", "Y\0a\0k"] {
            assert_eq!(
                parse(expired),
                Some((SignatureState::Expired, "Signed, key expired".into()))
            );
        }
        assert_eq!(
            parse("R\0\0"),
            Some((SignatureState::Revoked, "Signed, key revoked".into()))
        );
        assert_eq!(
            parse("E\0\0K"),
            Some((
                SignatureState::KeyMissing,
                "Signed (key not available)".into()
            ))
        );
        assert_eq!(
            parse("B\0x\0k"),
            Some((SignatureState::Bad, "Bad signature".into()))
        );
        assert_eq!(parse("N\0\0\n"), None);
        assert_eq!(parse(""), None);
        assert_eq!(parse_signature("G\0\0KEY").unwrap().text(), "Signed");
        assert_eq!(parse_signature("G\0x\0KEY").unwrap().key, "KEY");
    }
}
