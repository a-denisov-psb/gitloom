//! Stash message template (M10-10): presets, placeholders and rendering.
//! Pure; the stash push uses the result when no message was entered.

/// Presets of the dropdown, the first one is the default.
pub const PRESETS: [&str; 5] = [
    "{branch} · {datetime}",
    "WIP on {branch} · {datetime}",
    "{datetime} · {branch}",
    "{branch}: {summary}",
    "{branch}/{datetime}",
];

/// Hint shown for a preset, if it has one.
pub fn preset_hint(template: &str) -> Option<&'static str> {
    (template == "{branch}/{datetime}").then_some("groups stashes by branch in the STASHES section")
}

/// What the placeholders stand for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    /// Checked-out branch; `None` for a detached HEAD.
    pub branch: Option<String>,
    /// Short id of HEAD.
    pub sha: String,
    /// Summary of the HEAD commit.
    pub summary: String,
    /// Current time as Unix seconds (shown like the graph's dates).
    pub now: i64,
}

/// The rendered message, or why the template cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rendered {
    Message(String),
    /// Empty template: Git's default message is used.
    Empty,
    /// `{x}` is not a placeholder.
    Unknown(String),
}

impl Rendered {
    /// The message for `git stash push -m`, if any.
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Message(message) => Some(message),
            Self::Empty | Self::Unknown(_) => None,
        }
    }

    /// Inline hint under the field.
    pub fn hint(&self) -> Option<String> {
        match self {
            Self::Message(_) => None,
            Self::Empty => Some("Git's default message is used".into()),
            Self::Unknown(name) => Some(format!("Unknown placeholder {{{name}}}")),
        }
    }
}

/// Replaces `{branch}`, `{date}`, `{time}`, `{datetime}`, `{sha}` and
/// `{summary}` in `template`.
pub fn render(template: &str, context: &Context) -> Rendered {
    if template.trim().is_empty() {
        return Rendered::Empty;
    }
    let datetime = crate::format::date_time(context.now);
    let (date, time) = datetime.split_once(' ').unwrap_or((&datetime, ""));
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let name = &after[..end];
        let value = match name {
            "branch" => context.branch.as_deref().unwrap_or("detached"),
            "date" => date,
            "time" => time,
            "datetime" => &datetime,
            "sha" => &context.sha,
            "summary" => &context.summary,
            _ => return Rendered::Unknown(name.to_owned()),
        };
        out.push_str(value);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    let message = out.trim().to_owned();
    if message.is_empty() {
        Rendered::Empty
    } else {
        Rendered::Message(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(branch: Option<&str>) -> Context {
        Context {
            branch: branch.map(str::to_owned),
            sha: "1a2b3c4".into(),
            summary: "Add login".into(),
            // 2026-11-11 10:42 UTC
            now: 1_794_393_720,
        }
    }

    #[test]
    fn every_placeholder() {
        let rendered = render(
            "{branch}|{date}|{time}|{datetime}|{sha}|{summary}",
            &context(Some("feature/x")),
        );
        assert_eq!(
            rendered,
            Rendered::Message(
                "feature/x|2026-11-11|10:42|2026-11-11 10:42|1a2b3c4|Add login".into()
            )
        );
        assert_eq!(
            render(PRESETS[0], &context(Some("main"))).message(),
            Some("main · 2026-11-11 10:42")
        );
    }

    #[test]
    fn detached_unknown_and_empty() {
        assert_eq!(
            render("{branch}: {summary}", &context(None)).message(),
            Some("detached: Add login")
        );
        let unknown = render("{branch} {user}", &context(Some("main")));
        assert_eq!(unknown, Rendered::Unknown("user".into()));
        assert_eq!(
            unknown.hint().as_deref(),
            Some("Unknown placeholder {user}")
        );
        assert_eq!(render("  ", &context(None)), Rendered::Empty);
        assert_eq!(
            Rendered::Empty.hint().as_deref(),
            Some("Git's default message is used")
        );
        // A lone brace is kept as text.
        assert_eq!(render("a { b", &context(None)).message(), Some("a { b"));
        assert!(preset_hint(PRESETS[4]).is_some());
        assert_eq!(preset_hint(PRESETS[0]), None);
    }
}
