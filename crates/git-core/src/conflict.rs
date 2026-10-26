//! Conflict resolution model (M4-05, ADR-022): a conflicted text file as
//! clean chunks and conflict hunks, the user's line picks from "ours" and
//! "theirs", and the output they produce. Pure: no I/O.
//!
//! Lines keep their terminator (`\n`, `\r\n` or none for the last line), so
//! the output reproduces the file's line endings byte for byte.

/// Marker length used when parsing `git merge-file` output; longer than the
/// usual 7 so that conflict markers committed in the file are not mistaken
/// for real ones.
pub const PARSE_MARKER_SIZE: usize = 31;

/// Marker length in the output shown to the user (Git's default).
const OUTPUT_MARKER_SIZE: usize = 7;

/// One part of a conflicted file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chunk {
    /// Merged without conflict.
    Clean(Vec<String>),
    Conflict(ConflictHunk),
}

/// Lines of both sides (and the merge base) where they conflict.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConflictHunk {
    pub ours: Vec<String>,
    pub base: Vec<String>,
    pub theirs: Vec<String>,
}

/// A conflicted text file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictText {
    pub chunks: Vec<Chunk>,
    /// Line terminator for added markers and joined lines (`\r\n` if the
    /// file uses CRLF).
    pub eol: &'static str,
}

/// Which side of a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Ours,
    Theirs,
}

/// Picked lines of one conflict hunk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HunkPicks {
    pub ours: Vec<bool>,
    pub theirs: Vec<bool>,
}

impl HunkPicks {
    fn new(hunk: &ConflictHunk) -> Self {
        Self {
            ours: vec![false; hunk.ours.len()],
            theirs: vec![false; hunk.theirs.len()],
        }
    }

    pub fn side(&self, side: Side) -> &[bool] {
        match side {
            Side::Ours => &self.ours,
            Side::Theirs => &self.theirs,
        }
    }

    fn side_mut(&mut self, side: Side) -> &mut Vec<bool> {
        match side {
            Side::Ours => &mut self.ours,
            Side::Theirs => &mut self.theirs,
        }
    }

    /// Nothing picked on either side: the conflict is still open.
    pub fn is_open(&self) -> bool {
        !self.ours.iter().chain(&self.theirs).any(|&p| p)
    }
}

/// How much of one side of a hunk is picked (for the hunk checkbox).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickState {
    None,
    Some,
    All,
}

/// The user's picks for every conflict hunk of a file, in hunk order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Picks {
    pub hunks: Vec<HunkPicks>,
}

impl Picks {
    /// Nothing picked.
    pub fn new(text: &ConflictText) -> Self {
        Self {
            hunks: text.hunks().map(HunkPicks::new).collect(),
        }
    }

    /// Toggles line `line` of `side` in hunk `hunk`.
    pub fn toggle_line(&mut self, hunk: usize, side: Side, line: usize) {
        if let Some(pick) = self
            .hunks
            .get_mut(hunk)
            .and_then(|h| h.side_mut(side).get_mut(line))
        {
            *pick = !*pick;
        }
    }

    /// Hunk checkbox: picks all lines of `side`, or none if all were picked.
    pub fn toggle_hunk(&mut self, hunk: usize, side: Side) {
        if let Some(picks) = self.hunks.get_mut(hunk).map(|h| h.side_mut(side)) {
            let all = picks.iter().all(|&p| p);
            picks.iter_mut().for_each(|p| *p = !all);
        }
    }

    pub fn hunk_state(&self, hunk: usize, side: Side) -> PickState {
        let Some(picks) = self.hunks.get(hunk).map(|h| h.side(side)) else {
            return PickState::None;
        };
        match picks.iter().filter(|&&p| p).count() {
            0 => PickState::None,
            n if n == picks.len() => PickState::All,
            _ => PickState::Some,
        }
    }

    /// "Take all ours/theirs": every line of `side`, nothing of the other.
    pub fn take_all(&mut self, side: Side) {
        for hunk in &mut self.hunks {
            hunk.ours.iter_mut().for_each(|p| *p = side == Side::Ours);
            hunk.theirs
                .iter_mut()
                .for_each(|p| *p = side == Side::Theirs);
        }
    }

    /// Conflicts nothing was picked for.
    pub fn open_count(&self) -> usize {
        self.hunks.iter().filter(|h| h.is_open()).count()
    }
}

impl ConflictText {
    /// Parses `git merge-file -p --diff3 --marker-size=PARSE_MARKER_SIZE`
    /// output. `None` if the markers are malformed.
    pub fn parse(merged: &str) -> Option<Self> {
        let open = "<".repeat(PARSE_MARKER_SIZE);
        let base = "|".repeat(PARSE_MARKER_SIZE);
        let middle = "=".repeat(PARSE_MARKER_SIZE);
        let close = ">".repeat(PARSE_MARKER_SIZE);
        let is_marker = |line: &str, marker: &str| {
            line.strip_prefix(marker)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '\r', '\n']))
        };

        #[derive(PartialEq)]
        enum Part {
            Clean,
            Ours,
            Base,
            Theirs,
        }
        let mut chunks = Vec::new();
        let mut clean = Vec::new();
        let mut hunk = ConflictHunk::default();
        let mut part = Part::Clean;
        for line in merged.split_inclusive('\n') {
            match part {
                Part::Clean if is_marker(line, &open) => {
                    if !clean.is_empty() {
                        chunks.push(Chunk::Clean(std::mem::take(&mut clean)));
                    }
                    part = Part::Ours;
                }
                Part::Clean => clean.push(line.to_owned()),
                Part::Ours if is_marker(line, &base) => part = Part::Base,
                Part::Ours | Part::Base if is_marker(line, &middle) => part = Part::Theirs,
                Part::Ours => hunk.ours.push(line.to_owned()),
                Part::Base => hunk.base.push(line.to_owned()),
                Part::Theirs if is_marker(line, &close) => {
                    chunks.push(Chunk::Conflict(std::mem::take(&mut hunk)));
                    part = Part::Clean;
                }
                Part::Theirs => hunk.theirs.push(line.to_owned()),
            }
        }
        if part != Part::Clean {
            return None;
        }
        if !clean.is_empty() {
            chunks.push(Chunk::Clean(clean));
        }
        let eol = if merged.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        Some(Self { chunks, eol })
    }

    /// Conflict hunks in order.
    pub fn hunks(&self) -> impl Iterator<Item = &ConflictHunk> {
        self.chunks.iter().filter_map(|chunk| match chunk {
            Chunk::Conflict(hunk) => Some(hunk),
            Chunk::Clean(_) => None,
        })
    }

    /// Output for `picks`: clean chunks, picked lines (ours, then theirs)
    /// and Git-style markers for conflicts nothing was picked for.
    pub fn output(&self, picks: &Picks, labels: (&str, &str)) -> String {
        let eol = self.eol;
        let marker = |c: &str, label: &str| {
            let mut line = c.repeat(OUTPUT_MARKER_SIZE);
            if !label.is_empty() {
                line.push(' ');
                line.push_str(label);
            }
            line.push_str(eol);
            line
        };
        let mut lines: Vec<String> = Vec::new();
        let mut hunk_index = 0;
        for chunk in &self.chunks {
            match chunk {
                Chunk::Clean(clean) => lines.extend(clean.iter().cloned()),
                Chunk::Conflict(hunk) => {
                    let empty = HunkPicks::new(hunk);
                    let pick = picks.hunks.get(hunk_index).unwrap_or(&empty);
                    hunk_index += 1;
                    let picked = |side: &[String], flags: &[bool]| {
                        side.iter()
                            .zip(flags)
                            .filter(|(_, &on)| on)
                            .map(|(line, _)| line.clone())
                            .collect::<Vec<_>>()
                    };
                    if pick.is_open() {
                        lines.push(marker("<", labels.0));
                        lines.extend(terminated(&hunk.ours, eol));
                        lines.push(marker("=", ""));
                        lines.extend(terminated(&hunk.theirs, eol));
                        lines.push(marker(">", labels.1));
                    } else {
                        lines.extend(picked(&hunk.ours, &pick.ours));
                        lines.extend(picked(&hunk.theirs, &pick.theirs));
                    }
                }
            }
        }
        join_lines(&lines, eol)
    }
}

/// `lines` where the last one gets a terminator if it has none (a side
/// without trailing newline followed by a marker).
fn terminated(lines: &[String], eol: &str) -> Vec<String> {
    let mut lines = lines.to_vec();
    if let Some(last) = lines.last_mut() {
        if !last.ends_with('\n') {
            last.push_str(eol);
        }
    }
    lines
}

/// Concatenates lines; a line without terminator that is not the last one
/// (the end of one side, followed by more lines) gets `eol`.
fn join_lines(lines: &[String], eol: &str) -> String {
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        out.push_str(line);
        if i + 1 < lines.len() && !line.ends_with('\n') {
            out.push_str(eol);
        }
    }
    out
}

/// Text edited in the output editor (always `\n`) with the file's line
/// endings restored.
pub fn with_line_endings(edited: &str, eol: &str) -> String {
    let normalized = edited.replace("\r\n", "\n");
    if eol == "\n" {
        normalized
    } else {
        normalized.replace('\n', eol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(c: char) -> String {
        c.to_string().repeat(PARSE_MARKER_SIZE)
    }

    /// merge-file style output with one conflict per `(ours, base, theirs)`
    /// between clean parts.
    fn merged(parts: &[&str], conflicts: &[(&str, &str, &str)], eol: &str) -> String {
        let mut out = String::new();
        for (i, clean) in parts.iter().enumerate() {
            out.push_str(clean);
            if let Some((ours, base, theirs)) = conflicts.get(i) {
                out.push_str(&format!("{} ours{eol}{ours}", marker('<')));
                out.push_str(&format!("{} base{eol}{base}", marker('|')));
                out.push_str(&format!("{}{eol}{theirs}", marker('=')));
                out.push_str(&format!("{} theirs{eol}", marker('>')));
            }
        }
        out
    }

    const LABELS: (&str, &str) = ("HEAD", "topic");

    #[test]
    fn parses_clean_parts_and_conflicts() {
        let text = merged(
            &["a\n", "m\n", "z\n"],
            &[("o1\no2\n", "b\n", "t1\n"), ("x\n", "", "y\n")],
            "\n",
        );
        let parsed = ConflictText::parse(&text).unwrap();
        assert_eq!(parsed.eol, "\n");
        assert_eq!(
            parsed.chunks,
            vec![
                Chunk::Clean(vec!["a\n".into()]),
                Chunk::Conflict(ConflictHunk {
                    ours: vec!["o1\n".into(), "o2\n".into()],
                    base: vec!["b\n".into()],
                    theirs: vec!["t1\n".into()],
                }),
                Chunk::Clean(vec!["m\n".into()]),
                Chunk::Conflict(ConflictHunk {
                    ours: vec!["x\n".into()],
                    base: vec![],
                    theirs: vec!["y\n".into()],
                }),
                Chunk::Clean(vec!["z\n".into()]),
            ]
        );
    }

    #[test]
    fn committed_short_markers_are_content() {
        let text = merged(&["<<<<<<< not a conflict\n=======\n"], &[], "\n");
        let parsed = ConflictText::parse(&text).unwrap();
        assert_eq!(parsed.hunks().count(), 0);
        assert_eq!(parsed.output(&Picks::new(&parsed), LABELS), text);
    }

    #[test]
    fn unclosed_conflict_is_rejected() {
        assert!(ConflictText::parse(&format!("{} ours\nx\n", marker('<'))).is_none());
    }

    #[test]
    fn open_conflicts_get_git_markers() {
        let text = merged(&["a\n", "z\n"], &[("o\n", "b\n", "t\n")], "\n");
        let parsed = ConflictText::parse(&text).unwrap();
        let picks = Picks::new(&parsed);
        assert_eq!(picks.open_count(), 1);
        assert_eq!(
            parsed.output(&picks, LABELS),
            "a\n<<<<<<< HEAD\no\n=======\nt\n>>>>>>> topic\nz\n"
        );
    }

    #[test]
    fn overlapping_hunks_resolve_independently() {
        let text = merged(
            &["a\n", "m\n", "z\n"],
            &[("o1\n", "b\n", "t1\n"), ("o2\n", "b\n", "t2\n")],
            "\n",
        );
        let parsed = ConflictText::parse(&text).unwrap();
        let mut picks = Picks::new(&parsed);
        picks.toggle_hunk(0, Side::Theirs);
        assert_eq!(picks.open_count(), 1);
        let out = parsed.output(&picks, LABELS);
        assert_eq!(
            out,
            "a\nt1\nm\n<<<<<<< HEAD\no2\n=======\nt2\n>>>>>>> topic\nz\n"
        );
        picks.toggle_hunk(1, Side::Ours);
        assert_eq!(parsed.output(&picks, LABELS), "a\nt1\nm\no2\nz\n");
    }

    #[test]
    fn line_picks_from_both_sides_keep_ours_first() {
        let text = merged(&["", ""], &[("o1\no2\no3\n", "", "t1\nt2\n")], "\n");
        let parsed = ConflictText::parse(&text).unwrap();
        let mut picks = Picks::new(&parsed);
        picks.toggle_line(0, Side::Theirs, 1);
        picks.toggle_line(0, Side::Ours, 0);
        picks.toggle_line(0, Side::Ours, 2);
        assert_eq!(picks.hunk_state(0, Side::Ours), PickState::Some);
        assert_eq!(picks.hunk_state(0, Side::Theirs), PickState::Some);
        assert_eq!(parsed.output(&picks, LABELS), "o1\no3\nt2\n");
        // Hunk checkbox with some lines picked: picks all; again: none.
        picks.toggle_hunk(0, Side::Ours);
        assert_eq!(picks.hunk_state(0, Side::Ours), PickState::All);
        picks.toggle_hunk(0, Side::Ours);
        assert_eq!(picks.hunk_state(0, Side::Ours), PickState::None);
        assert_eq!(parsed.output(&picks, LABELS), "t2\n");
    }

    #[test]
    fn take_all_picks_one_side_everywhere() {
        let text = merged(
            &["a\n", "m\n", ""],
            &[("o1\n", "", "t1\n"), ("o2\n", "", "t2\n")],
            "\n",
        );
        let parsed = ConflictText::parse(&text).unwrap();
        let mut picks = Picks::new(&parsed);
        picks.toggle_line(0, Side::Ours, 0);
        picks.take_all(Side::Theirs);
        assert_eq!(parsed.output(&picks, LABELS), "a\nt1\nm\nt2\n");
        picks.take_all(Side::Ours);
        assert_eq!(parsed.output(&picks, LABELS), "a\no1\nm\no2\n");
    }

    #[test]
    fn crlf_files_keep_crlf() {
        let text = merged(&["a\r\n", "z\r\n"], &[("o\r\n", "b\r\n", "t\r\n")], "\r\n");
        let parsed = ConflictText::parse(&text).unwrap();
        assert_eq!(parsed.eol, "\r\n");
        let mut picks = Picks::new(&parsed);
        assert_eq!(
            parsed.output(&picks, LABELS),
            "a\r\n<<<<<<< HEAD\r\no\r\n=======\r\nt\r\n>>>>>>> topic\r\nz\r\n"
        );
        picks.toggle_line(0, Side::Ours, 0);
        picks.toggle_line(0, Side::Theirs, 0);
        assert_eq!(parsed.output(&picks, LABELS), "a\r\no\r\nt\r\nz\r\n");
        assert_eq!(with_line_endings("a\no\n", parsed.eol), "a\r\no\r\n");
        assert_eq!(with_line_endings("a\r\no\n", "\n"), "a\no\n");
    }

    #[test]
    fn missing_trailing_newline() {
        // Conflict at the end of the file; neither side ends with a newline.
        let parsed = ConflictText {
            chunks: vec![
                Chunk::Clean(vec!["a\n".into()]),
                Chunk::Conflict(ConflictHunk {
                    ours: vec!["o".into()],
                    base: vec![],
                    theirs: vec!["t".into()],
                }),
            ],
            eol: "\n",
        };
        let mut picks = Picks::new(&parsed);
        assert_eq!(
            parsed.output(&picks, LABELS),
            "a\n<<<<<<< HEAD\no\n=======\nt\n>>>>>>> topic\n"
        );
        picks.toggle_line(0, Side::Ours, 0);
        assert_eq!(parsed.output(&picks, LABELS), "a\no");
        picks.toggle_line(0, Side::Theirs, 0);
        assert_eq!(parsed.output(&picks, LABELS), "a\no\nt");
    }
}
