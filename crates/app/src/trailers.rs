//! Co-authors from `Co-authored-by:` trailers of a commit message (M14-02).
//! Pure: no iced types.

/// A co-author of a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoAuthor {
    pub name: String,
    pub email: String,
}

/// Co-authors named in the trailers of `body`: lines `Co-authored-by: Name
/// <mail>` (key case-insensitive) in its last paragraph, like Git's trailer
/// block. Malformed lines are skipped; duplicates are listed once.
pub fn co_authors(body: &str) -> Vec<CoAuthor> {
    let last = body.trim_end().rsplit("\n\n").next().unwrap_or_default();
    let mut found: Vec<CoAuthor> = Vec::new();
    for line in last.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("co-authored-by") {
            continue;
        }
        if let Some(author) = parse_identity(value.trim()) {
            if !found.contains(&author) {
                found.push(author);
            }
        }
    }
    found
}

/// `Name <mail>`: a non-empty name and a mail with `@` in angle brackets
/// at the end.
fn parse_identity(value: &str) -> Option<CoAuthor> {
    let open = value.rfind('<')?;
    let email = value[open + 1..].strip_suffix('>')?.trim();
    let name = value[..open].trim();
    if name.is_empty() || !email.contains('@') || email.contains(['<', '>', ' ']) {
        return None;
    }
    Some(CoAuthor {
        name: name.to_owned(),
        email: email.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(body: &str) -> Vec<String> {
        co_authors(body)
            .into_iter()
            .map(|a| format!("{} <{}>", a.name, a.email))
            .collect()
    }

    #[test]
    fn trailers_of_the_last_paragraph() {
        let body = "Explain the change.\n\nCo-authored-by: Ana Lyst <ana@example.com>\nco-authored-BY:  Bo Kim <bo@example.com>\nSigned-off-by: Cy <cy@example.com>";
        assert_eq!(
            names(body),
            ["Ana Lyst <ana@example.com>", "Bo Kim <bo@example.com>"]
        );
    }

    #[test]
    fn trailers_outside_the_last_paragraph_are_ignored() {
        let body = "Co-authored-by: Ana Lyst <ana@example.com>\n\nMore text after it.";
        assert!(co_authors(body).is_empty());
        // Trailing blank lines do not start a new paragraph.
        let body = "Text.\n\nCo-authored-by: Ana Lyst <ana@example.com>\n\n";
        assert_eq!(names(body), ["Ana Lyst <ana@example.com>"]);
        // A body that is only trailers.
        assert_eq!(names("Co-authored-by: Ana <a@x.org>"), ["Ana <a@x.org>"]);
    }

    #[test]
    fn malformed_lines_and_duplicates() {
        let body = "Co-authored-by: <nobody@example.com>\nCo-authored-by: No Mail\nCo-authored-by: Bad <not-a-mail>\nCo-authored-by: Open <a@x.org\nCo-authored by: Typo <t@x.org>\nCo-authored-by: Ana <a@x.org>\nCo-authored-by: Ana <a@x.org>";
        assert_eq!(names(body), ["Ana <a@x.org>"]);
        assert!(co_authors("").is_empty());
    }
}
