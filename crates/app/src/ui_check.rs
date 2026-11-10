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

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_file_history_split() {
        let mut app = crate::app::tests::file_history_app(crate::settings::DiffMode::Split);
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            snapshot(&app, &format!("file-history-split-{theme}"));
        }
    }

    /// M7-05: opening and scrolling a 2 MB / 50 000 line diff in every
    /// mode. Run with `--release`; prints the timings for PERFORMANCE.md.
    #[test]
    #[ignore = "manual timing; run with --release --ignored --nocapture"]
    fn perf_large_diff_modes() {
        use crate::settings::DiffMode;
        for mode in DiffMode::ALL {
            let (mut app, open) = crate::app::tests::large_diff_app(mode, 50_000);
            let rows = crate::app::tests::diff_rows(&app);
            // Scrolled to the right as well (M7-16): the text is moved.
            let _ = app.update(Message::DiffWheel(120.0));
            // A frame: build the view and lay it out (what iced does per
            // redraw before drawing), at 60 positions over the whole file.
            let mut frames = Vec::new();
            for step in 0..60 {
                let offset = step as f32 / 59.0 * rows as f32 * crate::ui::diff::ROW_HEIGHT;
                crate::app::tests::scroll_diff(&mut app, offset);
                let start = std::time::Instant::now();
                let sim = simulator(&app);
                drop(sim);
                frames.push(start.elapsed());
            }
            frames.sort();
            eprintln!(
                "{:6}: open {:>7.1?} ({rows} rows), frame median {:>6.1?}, max {:>6.1?}",
                mode.label(),
                open,
                frames[frames.len() / 2],
                frames[frames.len() - 1]
            );
        }
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_gitflow_preferences() {
        let mut app = ready_app(&["c", "b", "a"]);
        let _ = app.update(Message::GitDetected(git_core::GitAvailability::Available(
            git_core::MIN_GIT_VERSION,
        )));
        let _ = app.update(Message::SettingsOpenedAt(
            crate::settings::Category::Gitflow,
        ));
        snapshot(&app, "gitflow-preferences");
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_gitflow_section() {
        let mut app = crate::app::tests::flow_app();
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            snapshot(&app, &format!("gitflow-section-{theme}"));
        }
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_gitflow_finish() {
        let mut app = crate::app::tests::flow_app();
        let _ = app.update(Message::FlowFinishRequested("feature/login".into()));
        snapshot(&app, "gitflow-finish-0-feature");
        let _ = app.update(Message::FormClosed);
        let _ = app.update(Message::FlowFinishRequested("release/1.2.0".into()));
        snapshot(&app, "gitflow-finish-1-release");
        // A finish waiting after a restart.
        let app = crate::app::tests::finishing_app(5, false);
        snapshot(&app, "gitflow-finish-2-banner");
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
        let mut app = crate::app::tests::flow_app();
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

    /// Graph rows with dashed and solid lines across rows (M6-23).
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_graph_rows() {
        let app = ready_app(&["e", "d", "c", "b", "a"]);
        snapshot(&app, "graph-rows");
    }

    /// GITFLOW sits between LOCAL and REMOTE (M7-15); the widget order is
    /// also the keyboard focus order. Layout only, part of every test run.
    #[test]
    fn gitflow_section_between_local_and_remote() {
        let app = crate::app::tests::flow_app();
        let mut sim = simulator(&app);
        let mut top = |label: &str| {
            let found = sim.find(label).unwrap_or_else(|e| panic!("{label}: {e}"));
            iced_test::selector::Bounded::bounds(&found).y
        };
        let order = [
            "LOCAL",
            "GITFLOW",
            "REMOTE",
            "TAGS",
            "STASHES",
            "SUBMODULES",
        ]
        .map(&mut top);
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    }

    /// Hunk and Split diffs scrolled to the right (M7-16): numbers,
    /// markers and headers stay, the text moves, the scrollbar shows.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_diff_horizontal_scroll() {
        use crate::settings::DiffMode;
        for (mode, name) in [(DiffMode::Hunk, "hunk"), (DiffMode::Split, "split")] {
            let mut app = crate::app::tests::wide_diff_app(mode);
            snapshot(&app, &format!("diff-hscroll-{name}-0"));
            let _ = app.update(Message::DiffWheel(240.0));
            snapshot(&app, &format!("diff-hscroll-{name}-1"));
        }
    }

    /// A UTF-16 file shown as text with its encoding; line actions are
    /// disabled (M7-17).
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_utf16_diff() {
        use crate::settings::DiffMode;
        for (mode, name) in [(DiffMode::Hunk, "hunk"), (DiffMode::Inline, "inline")] {
            let app = crate::app::tests::utf16_diff_app(mode);
            snapshot(&app, &format!("utf16-diff-{name}"));
        }
    }

    /// Graph lines (M7-18): merges, branch-offs, a stash and WIP, both
    /// themes at 100 % and 200 %.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_graph_lines() {
        let mut app = crate::app::tests::graph_app();
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            for (zoom, size) in [(100, SIZE), (200, (SIZE.0 / 2.0, SIZE.1 / 2.0))] {
                snapshot_sized(&app, &format!("graph-lines-{theme}-{zoom}"), size);
            }
        }
    }

    /// Preferences with the repository group (M8-01): "Repository · <name>"
    /// with a repository, disabled "Repository" group on the start screen.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_preferences_repository_group() {
        let mut app = crate::app::tests::flow_app();
        let _ = app.update(Message::SettingsOpenedAt(
            crate::settings::Category::Gitflow,
        ));
        let (mut start, _) = App::new(Vec::new(), None);
        let _ = start.update(checked(Shortcut::Settings, false));
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let choice = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            app.settings.appearance.theme = choice;
            start.settings.appearance.theme = choice;
            snapshot(&app, &format!("preferences-repository-{theme}"));
            snapshot(&start, &format!("preferences-start-{theme}"));
        }
    }
}
