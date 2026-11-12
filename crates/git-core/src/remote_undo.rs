//! Undo and redo of remote and upstream changes (M12-08, ADR-043): add,
//! remove (with every `remote.<name>.*` key, the remote-tracking refs and
//! the upstreams that pointed to it), rename, URL change, set and unset
//! upstream. Every undo and redo checks that the configuration is still as
//! the action (or the undo) left it.
//!
//! Blocking: runs `git`.

use crate::{Error, GitWriter, Oid, Result, RunOptions};

/// `(remote, merge)` of a branch's upstream configuration.
pub type Upstream = (String, String);

/// A remote-tracking ref: name, commit, symbolic target (e.g. `HEAD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackingRef {
    pub name: String,
    pub target: Oid,
    pub symref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteChange {
    Add {
        name: String,
        url: String,
    },
    Remove {
        name: String,
        /// `remote.<name>.*` keys with their values, in config order.
        config: Vec<(String, String)>,
        refs: Vec<TrackingRef>,
        /// Branches whose upstream used this remote.
        upstreams: Vec<(String, Upstream)>,
    },
    Rename {
        old: String,
        new: String,
    },
    SetUrl {
        name: String,
        old: String,
        new: String,
    },
    Upstream {
        branch: String,
        before: Option<Upstream>,
        after: Option<Upstream>,
    },
}

impl RemoteChange {
    /// Shown in the Undo/Redo tooltips.
    pub fn label(&self) -> String {
        match self {
            Self::Add { name, .. } => format!("Add remote {name}"),
            Self::Remove { name, .. } => format!("Remove remote {name}"),
            Self::Rename { old, new } => format!("Rename remote {old} → {new}"),
            Self::SetUrl { name, .. } => format!("Edit URL of {name}"),
            Self::Upstream { branch, after, .. } => match after {
                Some(_) => format!("Set upstream of {branch}"),
                None => format!("Unset upstream of {branch}"),
            },
        }
    }
}

fn read() -> RunOptions<'static> {
    RunOptions {
        read_only: true,
        ..RunOptions::default()
    }
}

fn changed(undo: bool) -> Error {
    Error::Backend(format!(
        "Cannot {}: the repository changed",
        if undo { "undo" } else { "redo" }
    ))
}

/// All keys of the repository's own config, in order.
fn local_config(git: &dyn GitWriter) -> Result<Vec<(String, String)>> {
    let output = match git.run(&["config", "--local", "--list", "-z"], read()) {
        Ok(output) => output,
        Err(Error::Command { code: Some(1), .. }) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    Ok(output
        .stdout
        .split(|&b| b == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let text = String::from_utf8_lossy(entry);
            match text.split_once('\n') {
                Some((key, value)) => (key.to_owned(), value.to_owned()),
                None => (text.into_owned(), String::new()),
            }
        })
        .collect())
}

fn remote_config(git: &dyn GitWriter, name: &str) -> Result<Vec<(String, String)>> {
    let prefix = format!("remote.{name}.").to_lowercase();
    Ok(local_config(git)?
        .into_iter()
        .filter(|(key, _)| key.to_lowercase().starts_with(&prefix) && key.len() > prefix.len())
        .collect())
}

fn remote_url(git: &dyn GitWriter, name: &str) -> Result<Option<String>> {
    Ok(remote_config(git, name)?
        .into_iter()
        .find(|(key, _)| key.to_lowercase().ends_with(".url"))
        .map(|(_, value)| value))
}

fn exists(git: &dyn GitWriter, name: &str) -> Result<bool> {
    Ok(!remote_config(git, name)?.is_empty())
}

/// The upstream configuration of `branch`.
pub fn upstream_of(git: &dyn GitWriter, branch: &str) -> Result<Option<Upstream>> {
    let config = local_config(git)?;
    let get = |key: &str| {
        let key = format!("branch.{branch}.{key}").to_lowercase();
        config
            .iter()
            .rev()
            .find(|(k, _)| k.to_lowercase() == key)
            .map(|(_, v)| v.clone())
    };
    Ok(get("remote").zip(get("merge")))
}

fn set_config(git: &dyn GitWriter, key: &str, value: &str) -> Result<()> {
    git.run(&["config", "--local", key, value], RunOptions::default())
        .map(drop)
}

fn unset_config(git: &dyn GitWriter, key: &str) -> Result<()> {
    match git.run(
        &["config", "--local", "--unset-all", key],
        RunOptions::default(),
    ) {
        // 5: the key was not set.
        Ok(_) | Err(Error::Command { code: Some(5), .. }) => Ok(()),
        Err(error) => Err(error),
    }
}

fn write_upstream(git: &dyn GitWriter, branch: &str, upstream: Option<&Upstream>) -> Result<()> {
    let (remote_key, merge_key) = (
        format!("branch.{branch}.remote"),
        format!("branch.{branch}.merge"),
    );
    match upstream {
        Some((remote, merge)) => {
            set_config(git, &remote_key, remote)?;
            set_config(git, &merge_key, merge)
        }
        None => {
            unset_config(git, &remote_key)?;
            unset_config(git, &merge_key)
        }
    }
}

fn tracking_refs(git: &dyn GitWriter, name: &str) -> Result<Vec<TrackingRef>> {
    let output = git.run(
        &[
            "for-each-ref",
            "--format=%(refname)%00%(objectname)%00%(symref)",
            &format!("refs/remotes/{name}/"),
        ],
        read(),
    )?;
    Ok(output
        .stdout_text()
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\0');
            let name = fields.next()?.to_owned();
            let target = fields.next()?.to_owned();
            let symref = fields.next().filter(|s| !s.is_empty()).map(str::to_owned);
            Some(TrackingRef {
                name,
                target,
                symref,
            })
        })
        .collect())
}

/// Branches whose upstream uses remote `name`.
fn upstreams_using(git: &dyn GitWriter, name: &str) -> Result<Vec<(String, Upstream)>> {
    let config = local_config(git)?;
    let mut list = Vec::new();
    for (key, value) in &config {
        let Some(branch) = key
            .strip_prefix("branch.")
            .and_then(|rest| rest.strip_suffix(".remote"))
        else {
            continue;
        };
        if value != name {
            continue;
        }
        if let Some(upstream) = upstream_of(git, branch)? {
            list.push((branch.to_owned(), upstream));
        }
    }
    Ok(list)
}

/// Adds remote `name`, recorded for undo.
pub fn add_remote_recorded(git: &dyn GitWriter, name: &str, url: &str) -> Result<RemoteChange> {
    git.add_remote(name, url)?;
    Ok(RemoteChange::Add {
        name: name.to_owned(),
        url: url.to_owned(),
    })
}

/// Removes remote `name` after recording its config, tracking refs and
/// the upstreams using it.
pub fn remove_remote_recorded(git: &dyn GitWriter, name: &str) -> Result<RemoteChange> {
    let config = remote_config(git, name)?;
    let refs = tracking_refs(git, name)?;
    let upstreams = upstreams_using(git, name)?;
    git.remove_remote(name)?;
    Ok(RemoteChange::Remove {
        name: name.to_owned(),
        config,
        refs,
        upstreams,
    })
}

/// Changes the URL of `name`, recorded for undo.
pub fn set_url_recorded(git: &dyn GitWriter, name: &str, url: &str) -> Result<RemoteChange> {
    let old = remote_url(git, name)?.unwrap_or_default();
    git.set_remote_url(name, url)?;
    Ok(RemoteChange::SetUrl {
        name: name.to_owned(),
        old,
        new: url.to_owned(),
    })
}

/// Sets (`Some`) or unsets the upstream of `branch`, recorded for undo.
pub fn upstream_recorded(
    git: &dyn GitWriter,
    branch: &str,
    upstream: Option<&str>,
) -> Result<RemoteChange> {
    let before = upstream_of(git, branch)?;
    match upstream {
        Some(remote_ref) => git.set_upstream(branch, remote_ref)?,
        None => git.unset_upstream(branch)?,
    }
    let after = upstream_of(git, branch)?;
    Ok(RemoteChange::Upstream {
        branch: branch.to_owned(),
        before,
        after,
    })
}

fn restore_removed(
    git: &dyn GitWriter,
    config: &[(String, String)],
    refs: &[TrackingRef],
    upstreams: &[(String, Upstream)],
) -> Result<()> {
    for (key, value) in config {
        git.run(
            &["config", "--local", "--add", key, value],
            RunOptions::default(),
        )?;
    }
    for reference in refs.iter().filter(|r| r.symref.is_none()) {
        git.run(
            &["update-ref", &reference.name, &reference.target],
            RunOptions::default(),
        )?;
    }
    for reference in refs {
        if let Some(target) = &reference.symref {
            git.run(
                &["symbolic-ref", &reference.name, target],
                RunOptions::default(),
            )?;
        }
    }
    for (branch, upstream) in upstreams {
        write_upstream(git, branch, Some(upstream))?;
    }
    Ok(())
}

/// Undoes (`undo`) or redoes `change`; refuses without changes when the
/// configuration is no longer as expected.
pub fn switch_remote_change(git: &dyn GitWriter, change: &RemoteChange, undo: bool) -> Result<()> {
    match change {
        RemoteChange::Add { name, url } => {
            if undo {
                if remote_url(git, name)?.as_deref() != Some(url.as_str()) {
                    return Err(changed(true));
                }
                git.remove_remote(name)
            } else {
                if exists(git, name)? {
                    return Err(changed(false));
                }
                git.add_remote(name, url)
            }
        }
        RemoteChange::Remove {
            name,
            config,
            refs,
            upstreams,
        } => {
            if undo {
                if exists(git, name)? {
                    return Err(changed(true));
                }
                restore_removed(git, config, refs, upstreams)
            } else {
                if remote_config(git, name)? != *config {
                    return Err(changed(false));
                }
                git.remove_remote(name)
            }
        }
        RemoteChange::Rename { old, new } => {
            let (from, to) = if undo { (new, old) } else { (old, new) };
            if !exists(git, from)? || exists(git, to)? {
                return Err(changed(undo));
            }
            git.rename_remote(from, to)
        }
        RemoteChange::SetUrl { name, old, new } => {
            let (expected, target) = if undo { (new, old) } else { (old, new) };
            if remote_url(git, name)?.as_deref() != Some(expected.as_str()) {
                return Err(changed(undo));
            }
            git.set_remote_url(name, target)
        }
        RemoteChange::Upstream {
            branch,
            before,
            after,
        } => {
            let (expected, target) = if undo {
                (after, before)
            } else {
                (before, after)
            };
            if upstream_of(git, branch)? != *expected {
                return Err(changed(undo));
            }
            write_upstream(git, branch, target.as_ref())
        }
    }
}
