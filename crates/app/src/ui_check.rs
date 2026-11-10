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

    /// Menu items must stay inside their menu (the layout iced computes,
    /// with the fonts of this machine): long labels widen the menu, too
    /// long ones end in "…". Runs without a renderer, so it is part of
    /// every test run.
    #[test]
    fn context_menu_labels_stay_inside_the_menu() {
        use crate::overlay::{ContextMenu, MenuEntry, MENU_MAX_WIDTH};
        let mut app = ready_app(&["c"]);
        // Fonts are loaded with the first simulator (in the app: at start),
        // before any menu is measured.
        drop(simulator(&app));
        let long = "Merge origin/feature/sidebar-tree-with-folders into feature/ui/toolbar-pickers";
        let longest = format!("Rebase {long} onto {long}");
        let labels = ["Copy the SHA of this commit", long, longest.as_str()];
        let entries = labels
            .iter()
            .map(|label| MenuEntry::item(*label, Message::DismissNotice))
            .chain([MenuEntry::Item {
                label: "Fetch all remotes now".into(),
                shortcut: Some("Ctrl+Alt+Shift+L".into()),
                enabled: true,
                message: Box::new(Message::DismissNotice),
                hint: None,
            }])
            .collect();
        let menu = ContextMenu::new(iced::Point::new(40.0, 60.0), entries);
        assert!(menu.width > crate::overlay::MENU_MIN_WIDTH, "widened");
        assert_eq!(menu.width, MENU_MAX_WIDTH, "capped");
        let origin = menu.origin(iced::Size::new(SIZE.0, SIZE.1));
        let right = origin.x + menu.width;
        let shown: Vec<String> = menu
            .entries
            .iter()
            .filter_map(|e| match e {
                MenuEntry::Item { label, .. } => Some(label.clone()),
                MenuEntry::Separator => None,
            })
            .collect();
        assert!(shown[2].ends_with('…'), "{}", shown[2]);
        app.context_menu = Some(menu);
        let mut sim = simulator(&app);
        let shortcut = "Ctrl+Alt+Shift+L".to_owned();
        for label in shown.iter().chain(std::iter::once(&shortcut)) {
            let found = sim
                .find(label.as_str())
                .unwrap_or_else(|e| panic!("{label}: {e}"));
            let bounds = iced_test::selector::Bounded::bounds(&found);
            // iced clamps a text's layout to the room it gets: text that
            // does not fit is cut off. The full text must fit its bounds.
            let size = if *label == shortcut {
                crate::overlay::MENU_SHORTCUT_SIZE
            } else {
                crate::overlay::MENU_LABEL_SIZE
            };
            let needed = crate::overlay::text_width(label, size);
            assert!(
                needed <= bounds.width + 0.5,
                "{label:?} needs {needed} px but gets {}",
                bounds.width
            );
            assert!(
                bounds.x + bounds.width <= right - 4.0,
                "{label:?} ends at {} but the menu at {right}",
                bounds.x + bounds.width
            );
        }
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_wide_context_menu() {
        let mut app = crate::app::tests::menu_app();
        let _ = app.update(Message::BranchContextMenu {
            name: "feature/ui/toolbar".into(),
            kind: git_core::RefKind::LocalBranch,
            position: iced::Point::new(300.0, 120.0),
        });
        snapshot(&app, "context-menu-wide");
    }

    /// The section counts of a scrolling left panel end left of its
    /// scrollbar (M6-22). Layout only, so part of every test run.
    #[test]
    fn left_panel_counts_stay_clear_of_the_scrollbar() {
        let app = crate::app::tests::crowded_app();
        let mut sim = simulator(&app);
        let count = sim.find("57").unwrap_or_else(|e| panic!("no count: {e}"));
        let count = iced_test::selector::Bounded::bounds(&count);
        // The filter input sits 8 px inside the panel on both sides.
        let filter = sim
            .find(iced::widget::Id::new(crate::app::FILTER_INPUT))
            .unwrap_or_else(|e| panic!("no filter: {e}"));
        let filter = iced_test::selector::Bounded::bounds(&filter);
        let panel_right = filter.x + filter.width + 8.0;
        // Default scrollbar width of iced.
        let scrollbar_left = panel_right - 10.0;
        assert!(
            count.x + count.width + crate::ui::SCROLLBAR_GAP <= scrollbar_left,
            "count ends at {} but the scrollbar starts at {scrollbar_left}",
            count.x + count.width
        );
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_left_panel_scrollbar_both_themes() {
        let mut app = crate::app::tests::crowded_app();
        for (light, name) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            snapshot(&app, &format!("left-panel-scrollbar-{name}"));
        }
    }
}
