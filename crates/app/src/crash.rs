//! Local crash reports (M6-05, ADR-029). A panic hook writes
//! `<data dir>/crashes/<UTC timestamp>.txt` with version, commit, OS, thread,
//! panic message, location and backtrace. Nothing is uploaded; the next
//! start offers the newest report once.
//!
//! The panic message is written as is: if it contains a path (e.g. from a
//! `{path:?}` in an error), that path is in the report. The report never
//! adds repository paths, file contents or emails on its own.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use git_core::identity;

/// Reports kept in the directory; older ones are deleted.
pub const KEEP: usize = 20;

/// Names of the reports already offered, one per line.
const SHOWN_FILE: &str = "shown.txt";

/// `<data dir>/crashes`.
pub fn default_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", identity::DIR_NAME)
        .map(|dirs| dirs.data_dir().join("crashes"))
}

/// What a report contains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub version: String,
    pub commit: String,
    pub os: String,
    pub arch: String,
    pub thread: String,
    pub message: String,
    pub location: String,
    pub backtrace: String,
    /// UTC, e.g. `2026-11-06 10:55:03 UTC`.
    pub time: String,
}

impl Report {
    pub fn format(&self) -> String {
        format!(
            "{name} crash report\n\
             \n\
             Version:   {version}\n\
             Commit:    {commit}\n\
             OS:        {os} ({arch})\n\
             Time:      {time}\n\
             Thread:    {thread}\n\
             Location:  {location}\n\
             \n\
             Message:\n{message}\n\
             \n\
             Backtrace:\n{backtrace}\n",
            name = identity::DISPLAY_NAME,
            version = self.version,
            commit = self.commit,
            os = self.os,
            arch = self.arch,
            time = self.time,
            thread = self.thread,
            location = self.location,
            message = self.message,
            backtrace = self.backtrace.trim_end(),
        )
    }
}

/// Days since 1970-01-01 to (year, month, day) in the proleptic Gregorian
/// calendar (H. Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// (date, time) parts in UTC of `secs` since the Unix epoch.
fn utc_parts(secs: u64) -> ((i64, u32, u32), (u64, u64, u64)) {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    (
        civil_from_days(days),
        (rest / 3600, rest % 3600 / 60, rest % 60),
    )
}

/// Human readable UTC time.
pub fn utc_text(time: SystemTime) -> String {
    let secs = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let ((y, mo, d), (h, mi, s)) = utc_parts(secs);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02} UTC")
}

/// File name of a report written at `time`, sortable and valid on every
/// platform: `2026-11-06T10-55-03.123Z.txt`.
pub fn file_name(time: SystemTime) -> String {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let ((y, mo, d), (h, mi, s)) = utc_parts(since.as_secs());
    format!(
        "{y:04}-{mo:02}-{d:02}T{h:02}-{mi:02}-{s:02}.{:03}Z.txt",
        since.subsec_millis()
    )
}

/// Report files in `dir`, oldest first.
fn reports(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with("Z.txt"))
        .collect();
    names.sort();
    names
}

/// Deletes all but the newest `keep` reports.
pub fn rotate(dir: &Path, keep: usize) {
    let names = reports(dir);
    let excess = names.len().saturating_sub(keep);
    for name in &names[..excess] {
        let _ = std::fs::remove_file(dir.join(name));
    }
}

/// Writes `report` to `dir` and rotates; returns the file.
pub fn write(dir: &Path, report: &Report, time: SystemTime) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(file_name(time));
    std::fs::write(&path, report.format())?;
    rotate(dir, KEEP);
    Ok(path)
}

/// A report to offer on start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub path: PathBuf,
    pub text: String,
    /// The app ended (panic on the main thread).
    pub fatal: bool,
}

/// The newest report not offered yet; marks every current report as
/// offered, so each one is shown once.
pub fn take_pending(dir: &Path) -> Option<Pending> {
    let names = reports(dir);
    let shown_path = dir.join(SHOWN_FILE);
    let shown = std::fs::read_to_string(&shown_path).unwrap_or_default();
    let newest_unseen = names
        .iter()
        .rev()
        .find(|name| !shown.lines().any(|line| line == name.as_str()))
        .cloned();
    let newest = newest_unseen?;
    // Only names that still exist, so the file stays small.
    let _ = std::fs::write(&shown_path, names.join("\n") + "\n");
    let path = dir.join(&newest);
    let text = std::fs::read_to_string(&path).ok()?;
    let fatal = text
        .lines()
        .any(|line| line.strip_prefix("Thread:").map(str::trim) == Some("main"));
    Some(Pending { path, text, fatal })
}

/// Text of a panic payload.
fn payload_text(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "(non-text panic payload)".to_owned()
    }
}

/// Installs the panic hook (main process only, not in askpass or editor
/// mode). The previous hook still runs (message on stderr).
pub fn install(dir: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let report = Report {
            version: crate::about::version().to_owned(),
            commit: crate::about::commit().to_owned(),
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            thread: thread.name().unwrap_or("unnamed").to_owned(),
            message: payload_text(info.payload()),
            location: info
                .location()
                .map(|l| format!("{}:{}", l.file(), l.line()))
                .unwrap_or_default(),
            backtrace: std::backtrace::Backtrace::force_capture().to_string(),
            time: utc_text(SystemTime::now()),
        };
        let _ = write(&dir, &report, SystemTime::now());
        previous(info);
    }));
}

/// Debug builds only: if [`identity::DEBUG_PANIC_ENV`] names a directory,
/// install the hook for it and panic at once (integration test of the hook).
#[cfg(debug_assertions)]
#[allow(clippy::panic)] // The panic is the point: debug builds only, opt-in by env.
pub fn debug_panic_trigger() {
    if let Some(dir) = std::env::var_os(identity::DEBUG_PANIC_ENV) {
        install(PathBuf::from(dir));
        std::panic::panic_any("debug panic trigger");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn report(thread: &str) -> Report {
        Report {
            version: "1.0.0".into(),
            commit: "abc1234".into(),
            os: "linux".into(),
            arch: "x86_64".into(),
            thread: thread.into(),
            message: "index out of bounds".into(),
            location: "crates/app/src/x.rs:1".into(),
            backtrace: "   0: frame\n".into(),
            time: "2026-11-06 10:55:03 UTC".into(),
        }
    }

    #[test]
    fn report_text_has_every_field() {
        let text = report("main").format();
        for part in [
            "Version:   1.0.0",
            "Commit:    abc1234",
            "OS:        linux (x86_64)",
            "Time:      2026-11-06 10:55:03 UTC",
            "Thread:    main",
            "Location:  crates/app/src/x.rs:1",
            "Message:\nindex out of bounds",
            "Backtrace:\n   0: frame\n",
        ] {
            assert!(text.contains(part), "missing {part:?} in\n{text}");
        }
        assert!(text.starts_with(identity::DISPLAY_NAME));
    }

    #[test]
    fn panic_messages_are_written_as_is() {
        // Documented: a path in the message is not filtered.
        let mut r = report("main");
        r.message = "could not read /home/me/repo/file".into();
        assert!(r.format().contains("/home/me/repo/file"));
    }

    #[test]
    fn utc_names_and_texts() {
        let t = UNIX_EPOCH + Duration::from_millis(1_793_962_503_123);
        assert_eq!(file_name(t), "2026-11-06T10-55-03.123Z.txt");
        assert_eq!(utc_text(t), "2026-11-06 10:55:03 UTC");
        assert_eq!(file_name(UNIX_EPOCH), "1970-01-01T00-00-00.000Z.txt");
        // Leap day.
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(utc_text(leap), "2000-02-29 00:00:00 UTC");
        assert!(!file_name(t).contains(':'));
    }

    #[test]
    fn keeps_the_newest_twenty() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..25u64 {
            let t = UNIX_EPOCH + Duration::from_secs(1_700_000_000 + i);
            write(dir.path(), &report("main"), t).unwrap();
        }
        let names = reports(dir.path());
        assert_eq!(names.len(), KEEP);
        assert_eq!(
            names[0],
            file_name(UNIX_EPOCH + Duration::from_secs(1_700_000_005))
        );
        // Other files are left alone.
        std::fs::write(dir.path().join("notes.md"), "x").unwrap();
        rotate(dir.path(), 1);
        assert_eq!(reports(dir.path()).len(), 1);
        assert!(dir.path().join("notes.md").exists());
    }

    #[test]
    fn each_report_is_offered_once() {
        let dir = tempfile::tempdir().unwrap();
        assert!(take_pending(dir.path()).is_none());
        let first = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        write(dir.path(), &report("main"), first).unwrap();
        let pending = take_pending(dir.path()).unwrap();
        assert!(pending.fatal);
        assert!(pending.text.contains("index out of bounds"));
        assert!(pending.path.ends_with(file_name(first)));
        assert!(take_pending(dir.path()).is_none());

        // Two new ones: the newest is offered, both count as shown.
        write(
            dir.path(),
            &report("worker"),
            first + Duration::from_secs(1),
        )
        .unwrap();
        write(
            dir.path(),
            &report("worker"),
            first + Duration::from_secs(2),
        )
        .unwrap();
        let pending = take_pending(dir.path()).unwrap();
        assert!(!pending.fatal);
        assert!(pending
            .path
            .ends_with(file_name(first + Duration::from_secs(2))));
        assert!(take_pending(dir.path()).is_none());
    }

    #[test]
    fn hook_writes_a_report_for_a_caught_panic() {
        let dir = tempfile::tempdir().unwrap();
        let previous = std::panic::take_hook();
        install(dir.path().to_path_buf());
        let result = std::thread::Builder::new()
            .name("crash-test".into())
            .spawn(|| std::panic::panic_any("boom in test"))
            .unwrap()
            .join();
        std::panic::set_hook(previous);
        assert!(result.is_err());
        // Other tests may panic meanwhile; find ours.
        let text = reports(dir.path())
            .iter()
            .map(|name| std::fs::read_to_string(dir.path().join(name)).unwrap())
            .find(|text| text.contains("Thread:    crash-test"))
            .unwrap();
        assert!(text.contains("boom in test"));
        assert!(text.contains("Backtrace:"));
    }
}
