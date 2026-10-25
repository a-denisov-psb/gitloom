//! Interactive rebase plan (M4-07): actions per commit, validation, and the
//! todo list plus commit messages Git asks for (ADR-021). Pure: no I/O.

use crate::{EditorInput, Oid};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TodoAction {
    Pick,
    Reword,
    /// Meld into the previous commit, combining the messages.
    Squash,
    /// Meld into the previous commit, keeping its message.
    Fixup,
    Drop,
}

impl TodoAction {
    pub const ALL: [TodoAction; 5] = [
        Self::Pick,
        Self::Reword,
        Self::Squash,
        Self::Fixup,
        Self::Drop,
    ];

    /// Command in Git's todo list.
    pub fn command(self) -> &'static str {
        match self {
            Self::Pick => "pick",
            Self::Reword => "reword",
            Self::Squash => "squash",
            Self::Fixup => "fixup",
            Self::Drop => "drop",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pick => "Pick",
            Self::Reword => "Reword",
            Self::Squash => "Squash",
            Self::Fixup => "Fixup",
            Self::Drop => "Drop",
        }
    }

    /// Keyboard shortcut in the interactive rebase panel.
    pub fn key(self) -> char {
        match self {
            Self::Pick => 'p',
            Self::Reword => 'r',
            Self::Squash => 's',
            Self::Fixup => 'f',
            Self::Drop => 'd',
        }
    }

    fn melds(self) -> bool {
        matches!(self, Self::Squash | Self::Fixup)
    }
}

/// One commit of the plan, in todo order (oldest first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoEntry {
    pub id: Oid,
    pub action: TodoAction,
    /// Original full commit message.
    pub message: String,
}

/// A kept commit and the squash/fixup entries melded into it (indices into
/// the entries).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chain {
    pub head: usize,
    pub members: Vec<usize>,
}

impl Chain {
    fn has_squash(&self, entries: &[TodoEntry]) -> bool {
        self.members
            .iter()
            .any(|&i| entries[i].action == TodoAction::Squash)
    }
}

/// Why the plan cannot run, if it cannot.
pub fn validate(entries: &[TodoEntry]) -> Result<(), String> {
    let mut kept = entries.iter().filter(|e| e.action != TodoAction::Drop);
    match kept.next() {
        None => Err("Keep at least one commit".into()),
        Some(first) if first.action.melds() => Err(
            "The oldest kept commit cannot be squashed or fixed up: there is no commit to meld it into"
                .into(),
        ),
        Some(_) => Ok(()),
    }
}

/// Kept commits with the entries melded into them, in todo order.
pub fn chains(entries: &[TodoEntry]) -> Vec<Chain> {
    let mut chains: Vec<Chain> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        match entry.action {
            TodoAction::Drop => {}
            action if action.melds() => match chains.last_mut() {
                Some(chain) => chain.members.push(index),
                // Invalid plan (see `validate`); treated as its own chain.
                None => chains.push(Chain {
                    head: index,
                    members: Vec::new(),
                }),
            },
            _ => chains.push(Chain {
                head: index,
                members: Vec::new(),
            }),
        }
    }
    chains
}

/// The chain headed by entry `head` needs a message: a reword, or a chain
/// with at least one squash (one combined message).
pub fn needs_message(entries: &[TodoEntry], head: usize) -> bool {
    chains(entries)
        .iter()
        .find(|chain| chain.head == head)
        .is_some_and(|chain| {
            entries[head].action == TodoAction::Reword || chain.has_squash(entries)
        })
}

/// Prefill for the message of the chain headed by `head`: the head's
/// message, followed by those of the squashed (not fixed up) commits.
pub fn default_message(entries: &[TodoEntry], head: usize) -> String {
    let Some(chain) = chains(entries).into_iter().find(|c| c.head == head) else {
        return String::new();
    };
    std::iter::once(head)
        .chain(
            chain
                .members
                .iter()
                .copied()
                .filter(|&i| entries[i].action == TodoAction::Squash),
        )
        .map(|i| entries[i].message.trim())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Todo list and the messages in the order Git asks for them: one per
/// reword and one per chain with a squash (at the end of the chain).
/// `message(head)` returns the user's message for a chain needing one.
pub fn build(
    entries: &[TodoEntry],
    mut message: impl FnMut(usize) -> String,
) -> Result<EditorInput, String> {
    validate(entries)?;
    let chains = chains(entries);
    let mut messages = Vec::new();
    let mut reword_as_pick = Vec::new();
    for chain in &chains {
        if needs_message(entries, chain.head) {
            let text = message(chain.head);
            if text.trim().is_empty() {
                return Err(format!(
                    "Enter a message for {}",
                    short(&entries[chain.head].id)
                ));
            }
            messages.push(ensure_newline(text));
            // The combined message covers the head; rewording it as well
            // would ask twice.
            if chain.has_squash(entries) {
                reword_as_pick.push(chain.head);
            }
        }
    }
    let mut todo = String::new();
    for (index, entry) in entries.iter().enumerate() {
        let action = if reword_as_pick.contains(&index) {
            TodoAction::Pick
        } else {
            entry.action
        };
        todo.push_str(&format!("{} {}\n", action.command(), entry.id));
    }
    Ok(EditorInput {
        sequence: Some(todo),
        messages,
    })
}

fn short(id: &str) -> &str {
    &id[..id.len().min(7)]
}

fn ensure_newline(mut text: String) -> String {
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use TodoAction::*;

    fn plan(actions: &[TodoAction]) -> Vec<TodoEntry> {
        actions
            .iter()
            .enumerate()
            .map(|(i, &action)| TodoEntry {
                id: format!("c{i}"),
                action,
                message: format!("message {i}\n"),
            })
            .collect()
    }

    #[test]
    fn validation() {
        assert!(validate(&plan(&[Pick, Squash])).is_ok());
        assert_eq!(
            validate(&plan(&[Drop, Drop])).unwrap_err(),
            "Keep at least one commit"
        );
        assert!(validate(&plan(&[Squash, Pick])).is_err());
        // Dropped commits do not count as the oldest kept one.
        assert!(validate(&plan(&[Drop, Fixup, Pick])).is_err());
        assert!(validate(&plan(&[Drop, Reword, Fixup])).is_ok());
    }

    #[test]
    fn chains_group_melded_commits_and_skip_drops() {
        let entries = plan(&[Pick, Squash, Drop, Fixup, Reword, Pick]);
        assert_eq!(
            chains(&entries),
            vec![
                Chain {
                    head: 0,
                    members: vec![1, 3]
                },
                Chain {
                    head: 4,
                    members: vec![]
                },
                Chain {
                    head: 5,
                    members: vec![]
                },
            ]
        );
    }

    #[test]
    fn messages_are_needed_for_rewords_and_squash_chains() {
        let entries = plan(&[Pick, Fixup, Reword, Pick, Squash, Pick]);
        assert!(!needs_message(&entries, 0), "fixup keeps the message");
        assert!(needs_message(&entries, 2));
        assert!(needs_message(&entries, 3));
        assert!(!needs_message(&entries, 5));
        assert!(!needs_message(&entries, 4), "not a chain head");
        assert_eq!(default_message(&entries, 3), "message 3\n\nmessage 4");
        assert_eq!(default_message(&entries, 0), "message 0");
    }

    #[test]
    fn build_orders_messages_like_git_asks() {
        let entries = plan(&[Reword, Squash, Fixup, Pick, Reword, Drop]);
        let input = build(&entries, |head| format!("new {head}")).unwrap();
        assert_eq!(
            input.sequence.as_deref(),
            Some("pick c0\nsquash c1\nfixup c2\npick c3\nreword c4\ndrop c5\n")
        );
        assert_eq!(input.messages, ["new 0\n", "new 4\n"]);
    }

    #[test]
    fn build_rejects_invalid_plans_and_empty_messages() {
        assert!(build(&plan(&[Fixup, Pick]), |_| "x".into()).is_err());
        assert_eq!(
            build(&plan(&[Pick, Reword]), |_| "  \n".into()).unwrap_err(),
            "Enter a message for c1"
        );
    }
}
