//! Headless UI checks (M6-03): renders the real view with `iced_test`'s
//! simulator, clicks widgets by their text and writes PNG snapshots to
//! `target/ui-snapshots/`. The tests are ignored by default (they need a
//! renderer); run them with
//! `ICED_TEST_BACKEND=tiny-skia cargo test -p gitloom ui_check -- --ignored`.

use std::path::PathBuf;

use iced_test::Simulator;

use crate::app::{App, Message};

/// Window size of the checks (the default window of the app).
pub const SIZE: (f32, f32) = (1400.0, 860.0);

fn settings() -> iced::Settings {
    iced::Settings {
        default_text_size: 13.into(),
        ..iced::Settings::default()
    }
}

fn simulator(app: &App) -> Simulator<'_, Message> {
    Simulator::with_size(settings(), SIZE, app.view())
}

/// Directory of the snapshots.
pub fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-snapshots")
}

/// Snapshot path for `name`; older files of it are removed first (the
/// simulator compares against an existing file and appends the renderer
/// name, e.g. `<name>-tiny-skia.png`).
fn fresh_path(name: &str) -> PathBuf {
    for renderer in ["tiny-skia", "wgpu"] {
        let _ = std::fs::remove_file(snapshot_dir().join(format!("{name}-{renderer}.png")));
    }
    snapshot_dir().join(format!("{name}.png"))
}

/// Renders `app` to `target/ui-snapshots/<name>.png` (replacing an older one).
pub fn snapshot(app: &App, name: &str) -> PathBuf {
    let path = fresh_path(name);
    let mut sim = simulator(app);
    let snap = sim.snapshot(&app.theme()).unwrap();
    assert!(snap.matches_image(&path).unwrap());
    path
}

/// Renders `app` in a window of `size` logical pixels (a smaller size
/// stands for a higher zoom: the simulator always renders at scale 2).
pub fn snapshot_sized(app: &App, name: &str, size: (f32, f32)) -> PathBuf {
    let path = fresh_path(name);
    let mut sim = Simulator::with_size(settings(), size, app.view());
    let snap = sim.snapshot(&app.theme()).unwrap();
    assert!(snap.matches_image(&path).unwrap());
    path
}

/// Clicks the widget with `id` (focusing an input) and renders the result.
pub fn snapshot_focused(app: &App, id: &'static str, name: &str) -> PathBuf {
    let path = fresh_path(name);
    let mut sim = simulator(app);
    sim.click(iced::widget::Id::new(id))
        .unwrap_or_else(|e| panic!("no widget {id}: {e}"));
    let snap = sim.snapshot(&app.theme()).unwrap();
    assert!(snap.matches_image(&path).unwrap());
    path
}

/// Clicks the widget showing `label` and feeds the produced messages to `app`.
pub fn click(app: &mut App, label: &str) {
    let messages: Vec<Message> = {
        let mut sim = simulator(app);
        sim.click(label)
            .unwrap_or_else(|e| panic!("no clickable {label:?}: {e}"));
        sim.into_messages().collect()
    };
    assert!(!messages.is_empty(), "clicking {label:?} produced nothing");
    for message in messages {
        let _ = app.update(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{checked, ready_app};
    use crate::shortcuts::Shortcut;

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_about_dialog() {
        let mut app = ready_app(&["c", "b", "a"]);
        let _ = app.update(checked(Shortcut::Settings, false));
        snapshot(&app, "about-0-preferences");
        click(
            &mut app,
            &format!("About {}", git_core::identity::DISPLAY_NAME),
        );
        assert!(app.about);
        snapshot(&app, "about-1-dialog");
        click(&mut app, "Close");
        assert!(!app.about);
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_crash_notice() {
        let mut app = ready_app(&["c", "b", "a"]);
        let _ = app.update(Message::CrashReportFound(Some(crate::crash::Pending {
            path: "/tmp/crashes/x.txt".into(),
            text: "report".into(),
            fatal: true,
        })));
        snapshot(&app, "crash-notice");
        click(&mut app, "Copy report");
        assert_eq!(app.status, "Crash report copied");
        click(&mut app, "Dismiss");
        assert!(app.crash_notice.is_none());
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_update_notice_and_setting() {
        let mut app = ready_app(&["c", "b", "a"]);
        let _ = app.update(checked(Shortcut::Settings, false));
        snapshot(&app, "update-0-setting-off");
        let _ = app.update(Message::UpdateChecked(Ok(vec![crate::update::Release {
            version: crate::update::Version::parse("99.0.0").unwrap(),
            url: "https://example.com".into(),
        }])));
        let _ = app.update(Message::SettingsClosed);
        snapshot(&app, "update-1-notice");
        click(&mut app, "Dismiss");
        assert!(app.update_available.is_none());
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_focus_ring_both_themes() {
        let mut app = ready_app(&["c", "b", "a"]);
        let _ = app.update(checked(Shortcut::Settings, false));
        for (light, name) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            snapshot_focused(
                &app,
                crate::ui::settings::AUTO_FETCH_INPUT,
                &format!("focus-settings-{name}"),
            );
        }
        let _ = app.update(Message::SettingsClosed);
        let _ = app.update(Message::AddRemoteStart);
        snapshot_focused(
            &app,
            crate::ui::overlays::FORM_FIRST_INPUT,
            "focus-form-light",
        );
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_context_menu_keyboard() {
        let mut app = ready_app(&["c", "b", "a"]);
        let _ = app.update(Message::CommitSelected("b".into()));
        let _ = app.update(checked(Shortcut::ContextMenu, false));
        let _ = app.update(checked(Shortcut::Navigate(crate::nav::Move::Next), false));
        snapshot(&app, "context-menu-keyboard");
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_toolbar_pickers() {
        let mut app = ready_app(&["c", "b", "a"]);
        // Any update applies "Checking for Git…": the branch picker is disabled.
        let _ = app.update(Message::DismissNotice);
        assert!(app
            .action_block(crate::shortcuts::Shortcut::CheckoutBranch)
            .is_some());
        snapshot(&app, "pickers-0-disabled");
        let _ = app.update(Message::GitDetected(git_core::GitAvailability::Available(
            git_core::MIN_GIT_VERSION,
        )));
        snapshot(&app, "pickers-1-toolbar");
        click(&mut app, "repository");
        assert_eq!(
            app.palette.as_ref().map(|p| p.mode),
            Some(crate::palette::Mode::OpenRepository)
        );
        snapshot(&app, "pickers-2-repository");
        let _ = app.update(Message::PaletteClosed);
        click(&mut app, "branch");
        assert_eq!(
            app.palette.as_ref().map(|p| p.mode),
            Some(crate::palette::Mode::CheckoutBranch)
        );
        snapshot(&app, "pickers-3-branch");
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_left_panel_tree() {
        let mut app = crate::app::tests::sidebar_app();
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            // 200 %: half the logical size at the same pixel size.
            for (zoom, size) in [(100, SIZE), (200, (SIZE.0 / 2.0, SIZE.1 / 2.0))] {
                snapshot_sized(&app, &format!("left-panel-{theme}-{zoom}"), size);
            }
        }
        // A folder row toggles on click; "Show All" clears the hidden branch.
        click(&mut app, "ui");
        assert!(app.tabs[0]
            .view()
            .unwrap()
            .folders_collapsed
            .contains("local:feature/ui"));
        click(&mut app, "Show All");
        assert!(!app.tabs[0].view().unwrap().ref_filter.is_active());
        snapshot(&app, "left-panel-after-clicks");
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_diff_mode_toggle() {
        let mut app = crate::app::tests::app_with_open_diff(git_core::DiffKind::Unstaged);
        snapshot(&app, "diff-mode-0-hunk");
        click(&mut app, "Split");
        assert_eq!(
            app.settings.diff.view_mode,
            crate::settings::DiffMode::Split
        );
        snapshot(&app, "diff-mode-1-split-loading");
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_inline_diff() {
        let mut app = crate::app::tests::inline_diff_at_top();
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            snapshot(&app, &format!("inline-diff-{theme}"));
        }
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_split_diff() {
        use crate::settings::{DiffMode, ThemeChoice};
        let mut app = crate::app::tests::diff_at_top(DiffMode::Split, 6, 18);
        click_side(&mut app);
        let mut added = crate::app::tests::diff_at_top(DiffMode::Split, 0, 0);
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let choice = if light {
                ThemeChoice::Light
            } else {
                ThemeChoice::Dark
            };
            app.settings.appearance.theme = choice;
            added.settings.appearance.theme = choice;
            for (zoom, size) in [(100, SIZE), (200, (SIZE.0 / 2.0, SIZE.1 / 2.0))] {
                snapshot_sized(&app, &format!("split-diff-{theme}-{zoom}"), size);
            }
            snapshot(&added, &format!("split-diff-added-{theme}"));
        }
    }

    /// Selects the added line of the first changed row (right side).
    fn click_side(app: &mut App) {
        let row = app.tabs[0]
            .view()
            .and_then(|v| match &v.center {
                crate::repo_view::CenterView::Diff(diff) => diff.block_rows.first().copied(),
                _ => None,
            })
            .unwrap_or_default();
        let _ = app.update(Message::DiffSideClicked {
            row: row + 1,
            right: true,
        });
    }
}
