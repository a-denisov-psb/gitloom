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

/// Renders `app` at `scale` physical pixels per logical pixel (the
/// simulator always uses 2), e.g. 1.1 for a zoom of 110 % on a 100 %
/// display (M9-05). Writes `target/ui-snapshots/<name>.png`.
pub fn snapshot_scaled(app: &App, name: &str, scale: f32) -> PathBuf {
    use iced::advanced::renderer::Headless as _;
    use iced::theme::Base as _;
    use iced_test::runtime::user_interface::{Cache, UserInterface};

    // Loads the fonts like the app does at start.
    drop(simulator(app));
    let mut renderer = iced_test::futures::futures::executor::block_on(iced::Renderer::new(
        iced::Font::with_name("Fira Sans"),
        13.into(),
        Some("tiny-skia"),
    ))
    .expect("headless renderer");
    let theme = app.theme();
    let base = theme.base();
    let size = iced::Size::new(SIZE.0, SIZE.1);
    let mut ui = UserInterface::build(app.view(), size, Cache::default(), &mut renderer);
    ui.draw(
        &mut renderer,
        &theme,
        &iced::advanced::renderer::Style {
            text_color: base.text_color,
        },
        iced::mouse::Cursor::Unavailable,
    );
    let physical = iced::Size::new(
        (SIZE.0 * scale).round() as u32,
        (SIZE.1 * scale).round() as u32,
    );
    let rgba = renderer.screenshot(physical, scale, base.background_color);
    let pixmap = resvg::tiny_skia::IntSize::from_wh(physical.width, physical.height)
        .and_then(|size| resvg::tiny_skia::Pixmap::from_vec(rgba, size))
        .expect("screenshot size");
    let path = snapshot_dir().join(format!("{name}.png"));
    std::fs::create_dir_all(snapshot_dir()).expect("snapshot dir");
    pixmap.save_png(&path).expect("write snapshot");
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
                let offset = step as f32 / 59.0
                    * rows as f32
                    * crate::editor_style::EditorStyle::default().line_height();
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

    /// Banner of a stash pop that stopped in conflicts, after resolving
    /// and for conflicted files without an operation (M9-01), both themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_stash_conflict_banner() {
        use crate::app::tests::{pop_stash, resolve_all, staging_app, stash_app, with_conflicts};
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let choice = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            let mut app = stash_app();
            app.settings.appearance.theme = choice;
            pop_stash(&mut app, git_core::StashOutcome::Conflicts);
            with_conflicts(&mut app, &["a.txt"]);
            snapshot(&app, &format!("stash-conflict-0-{theme}"));
            resolve_all(&mut app);
            snapshot(&app, &format!("stash-conflict-1-resolved-{theme}"));
            let mut orphaned = staging_app(&[], &[]);
            orphaned.settings.appearance.theme = choice;
            with_conflicts(&mut orphaned, &["a.txt"]);
            snapshot(&orphaned, &format!("stash-conflict-2-orphaned-{theme}"));
        }
    }

    /// Icons at fractional scales (M9-05): left panel headers (with the "+"
    /// of REMOTE and GITFLOW), toolbar and file lists at 100, 110, 125 and
    /// 150 %, both themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_sharp_icons() {
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let mut app = crate::app::tests::flow_app();
            crate::app::tests::with_conflicts(&mut app, &["conflicted.txt"]);
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            for percent in [100_u32, 110, 125, 150] {
                let scale = percent as f32 / 100.0;
                let _ = app.update(Message::SystemScaleChanged(scale));
                snapshot_scaled(&app, &format!("sharp-icons-{percent}-{theme}"), scale);
            }
        }
    }

    /// Summary counter inside the field (M9-07): 69, 5 and −3 characters
    /// left, both themes; a long summary never runs under the counter.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_summary_counter() {
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let mut app = crate::app::tests::staging_app(&[], &["a.txt"]);
            let _ = app.update(Message::WipSelected);
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            for (left, summary) in [
                (69, "Fix".to_owned()),
                (5, "W".repeat(67)),
                (
                    -3,
                    "Summary that is far too long for one line ".repeat(2)[..75].to_owned(),
                ),
            ] {
                let _ = app.update(Message::SummaryChanged(summary));
                snapshot(&app, &format!("summary-counter-{left}-{theme}"));
            }
        }
    }

    /// Start screen with one recent repository (M10-01).
    fn start_app() -> App {
        let (mut app, _) = App::new(Vec::new(), None);
        let mut session = crate::session::Session::default();
        session.touch_recent(std::path::Path::new("/home/ana/projects/gitloom"), 0);
        session.touch_recent(std::path::Path::new("/home/ana/projects/website"), 1);
        let _ = app.update(crate::app::tests::loaded(session));
        app
    }

    /// Messages a click on the widget showing `label` produces (the
    /// window-wide drag end of every mouse release left out).
    fn clicked(app: &App, label: &str) -> Vec<Message> {
        let mut sim = simulator(app);
        sim.click(label)
            .unwrap_or_else(|e| panic!("no clickable {label:?}: {e}"));
        sim.into_messages()
            .filter(|m| !matches!(m, Message::BranchDragEnd))
            .collect()
    }

    /// Every entry of the start screen sends its message; Clone and Start
    /// stay disabled until M10-02/M10-03. Layout only, part of every run.
    #[test]
    fn start_screen_entries() {
        use crate::ui::start::{new_issue_url, release_notes_url, user_guide_url};
        let app = start_app();
        let one = |label: &str| {
            let messages = clicked(&app, label);
            assert_eq!(messages.len(), 1, "{label}: {messages:?}");
            messages.into_iter().next().unwrap()
        };
        assert!(matches!(one("Open a repo"), Message::OpenRepository));
        assert!(matches!(one("website"), Message::OpenRecent(p) if p.ends_with("website")));
        assert!(matches!(one("Preferences"), Message::SettingsOpened));
        assert!(matches!(
            one("Manage identities"),
            Message::SettingsOpenedAt(crate::settings::Category::Identities)
        ));
        assert!(matches!(one("User guide"), Message::OpenLink(u) if u == user_guide_url()));
        assert!(matches!(one("Report an issue"), Message::OpenLink(u) if u == new_issue_url()));
        assert!(matches!(one("Release notes"), Message::OpenLink(u) if u == release_notes_url()));
        assert!(matches!(one("Clone a repo"), Message::CloneStart));
        assert!(matches!(one("Start a local repo"), Message::InitStart));
        assert!(user_guide_url().starts_with(git_core::identity::REPOSITORY_URL));
        assert!(new_issue_url().ends_with("/issues/new"));
    }

    /// Clone dialog (M10-02): filled in, and running with progress.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_clone_dialog() {
        let mut app = start_app();
        let _ = app.update(Message::CloneStart);
        let _ = app.update(Message::CloneUrlChanged(
            "https://github.com/owner/tool.git".into(),
        ));
        snapshot(&app, "clone-dialog-0");
        if let Some(dialog) = app.clone_dialog.as_mut() {
            dialog.running = Some(crate::clone_dialog::CloneRun {
                cancel: git_core::CancelToken::new(),
                progress: Some(git_core::Progress {
                    phase: "Receiving objects".into(),
                    percent: Some(45),
                }),
            });
        }
        snapshot(&app, "clone-dialog-1-running");
    }

    /// Init dialog (M10-03) with a name and a `.gitignore` template.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_init_dialog() {
        let mut app = start_app();
        let _ = app.update(Message::InitStart);
        let _ = app.update(Message::InitNameChanged("project".into()));
        let _ = app.update(Message::InitGitignoreChosen("Rust".into()));
        snapshot(&app, "init-dialog");
        let _ = app.update(Message::InitClosed);
        let _ = app.update(Message::SettingsOpened);
        let _ = app.update(Message::DefaultBranchLoaded(Ok(Some("trunk".into()))));
        snapshot(&app, "preferences-general-default-branch");
    }

    /// Preferences → Signing (M10-04): GPG with keys and a repository
    /// override, then SSH.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_signing() {
        use git_core::signing::{GpgKey, SigningFormat, SigningSettings};
        let mut app = ready_app(&["c"]);
        let _ = app.update(Message::SettingsOpenedAt(
            crate::settings::Category::Signing,
        ));
        let mut page = crate::settings::SigningPage {
            global: SigningSettings {
                format: SigningFormat::Gpg,
                key: Some("AAAABBBBCCCCDDDD".into()),
                program: None,
                sign_commits: true,
                sign_tags: false,
            },
            overrides: vec![("commit.gpgSign".into(), "false".into())],
            gpg_keys: Ok(vec![
                GpgKey {
                    id: "AAAABBBBCCCCDDDD".into(),
                    user: "Ana Lyst <ana@example.com>".into(),
                    expires: None,
                    unusable: false,
                },
                GpgKey {
                    id: "EEEEFFFF00001111".into(),
                    user: "Old <old@example.com>".into(),
                    expires: Some(1),
                    unusable: true,
                },
            ]),
            ssh_keys: vec!["/home/ana/.ssh/id_ed25519.pub".into()],
        };
        let _ = app.update(Message::SigningLoaded(Ok(page.clone())));
        snapshot(&app, "signing-gpg");
        page.global.format = SigningFormat::Ssh;
        page.global.key = Some("/home/ana/.ssh/id_ed25519.pub".into());
        let _ = app.update(Message::SigningLoaded(Ok(page)));
        snapshot(&app, "signing-ssh");
    }

    /// Commit details with a good and a bad signature (M10-05).
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_signature_badge() {
        use git_core::signature::{Signature, SignatureState};
        for (state, name) in [(SignatureState::Good, "good"), (SignatureState::Bad, "bad")] {
            let mut app = ready_app(&["c", "b"]);
            let tab = app.tabs[0].id;
            let _ = app.update(Message::CommitSelected("b".into()));
            let _ = app.update(Message::SignatureLoaded {
                tab,
                id: "b".into(),
                result: Ok(Some(Signature {
                    state,
                    signer: "Ana Lyst <ana@example.com>".into(),
                    key: "AAAABBBBCCCCDDDD".into(),
                })),
            });
            snapshot(&app, &format!("signature-{name}"));
        }
    }

    /// Preferences → Repository → Commit (M10-06).
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_commit_template() {
        let mut app = ready_app(&["c"]);
        let _ = app.update(Message::SettingsOpenedAt(crate::settings::Category::Commit));
        let _ = app.update(Message::TemplateLoaded(Ok(
            git_core::commit_template::TemplateSettings {
                file: None,
                content: "feat(scope): \n\nWhy:\n# Explain the change\n".into(),
                options: Default::default(),
            },
        )));
        snapshot(&app, "commit-template");
    }

    /// Commit message prompt dialog and Preferences → Templates (M10-07).
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_commit_prompt() {
        let mut app = ready_app(&["c"]);
        let input = git_core::prompt::PromptInput {
            branch: "feature/login".into(),
            recent: vec!["Add login form".into(), "Set up the project".into()],
            files: vec![
                git_core::prompt::PromptFile::new(
                    "M",
                    "src/login.rs",
                    Some((3, 1)),
                    Ok("diff --git a/src/login.rs b/src/login.rs\n@@ -1,2 +1,4 @@\n-old\n+new\n+more\n+lines\n"
                        .into()),
                ),
                git_core::prompt::PromptFile::new(
                    "A",
                    "assets/logo.png",
                    None,
                    Err(git_core::prompt::Omitted::Binary),
                ),
            ],
        };
        let prompt = git_core::prompt::build(
            git_core::prompt::DEFAULT_INSTRUCTION,
            Some("feat: \n\nWhy:"),
            &input,
        );
        let _ = app.update(Message::PromptReady(Ok(prompt)));
        snapshot(&app, "commit-prompt");
        let _ = app.update(Message::PromptClosed);
        let _ = app.update(Message::SettingsOpenedAt(
            crate::settings::Category::Templates,
        ));
        snapshot(&app, "preferences-templates");
    }

    /// Toolbar without labels (M10-08).
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_toolbar_without_labels() {
        let mut app = ready_app(&["c"]);
        let _ = app.update(Message::ToolbarLabelsToggled(false));
        snapshot(&app, "toolbar-without-labels");
    }

    /// Preferences → Templates with the stash message template (M10-10):
    /// off, a preset, custom with an unknown placeholder; both themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_stash_template() {
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let mut app = ready_app(&["c"]);
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            let _ = app.update(Message::SettingsOpenedAt(
                crate::settings::Category::Templates,
            ));
            snapshot(&app, &format!("stash-template-0-off-{theme}"));
            let _ = app.update(Message::StashTemplateToggled(true));
            let _ = app.update(Message::StashTemplateChosen("{branch}/{datetime}".into()));
            snapshot(&app, &format!("stash-template-1-preset-{theme}"));
            let _ = app.update(Message::StashTemplateChosen(
                crate::app::STASH_CUSTOM.into(),
            ));
            let _ = app.update(Message::StashTemplateInput("{branch} {user}".into()));
            snapshot(&app, &format!("stash-template-2-custom-{theme}"));
        }
    }

    /// Preferences → Repository → External editor (M10-11).
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_repository_editor() {
        let mut app = ready_app(&["c"]);
        app.settings.tools.editor_command = "code {path}".into();
        let view = app.tabs[0].view_mut().unwrap();
        let mut repo = (*view.snapshot).clone();
        repo.repo_editor = Some("idea {path}".into());
        view.apply_snapshot(std::sync::Arc::new(repo));
        let _ = app.update(Message::SettingsOpenedAt(
            crate::settings::Category::RepoEditor,
        ));
        snapshot(&app, "repository-editor");
    }

    /// Classic Dark (M11-01): main window, Preferences and a dialog.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_classic_dark() {
        let mut app = crate::app::tests::flow_app();
        app.settings.appearance.theme = crate::settings::ThemeChoice::ClassicDark;
        snapshot(&app, "classic-dark-0-main");
        let _ = app.update(Message::SettingsOpenedAt(
            crate::settings::Category::Appearance,
        ));
        snapshot(&app, "classic-dark-1-preferences");
        let _ = app.update(Message::SettingsClosed);
        app.modal = Some(crate::overlay::Modal::confirm(
            "Delete branch",
            "Delete feature/login? It is merged into main.",
            "Delete",
            true,
            Message::DismissNotice,
        ));
        snapshot(&app, "classic-dark-2-dialog");
    }

    /// Classic Dark graph look (M11-04): row tint, filled pills with their
    /// connector (checked-out branch, remote and tag), a selected and a
    /// hovered row, several lanes, header separators and panel shadows.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_classic_dark_graph() {
        let reference = |name: &str, kind, target: &str| git_core::Reference {
            name: name.into(),
            kind,
            target: target.into(),
            is_head: false,
            upstream: None,
            ahead_behind: None,
        };
        let mut app = crate::app::tests::graph_app_with(vec![
            reference("origin/main", git_core::RefKind::RemoteBranch, "a"),
            reference("v1.0.0", git_core::RefKind::Tag, "b"),
        ]);
        app.settings.appearance.theme = crate::settings::ThemeChoice::ClassicDark;
        let _ = app.update(Message::CommitSelected("f".into()));
        let mut sim = simulator(&app);
        // Hover over the "Main work" row.
        let target = sim
            .find("Main work")
            .unwrap_or_else(|e| panic!("no row: {e}"));
        sim.point_at(iced_test::selector::Bounded::bounds(&target).center());
        let snap = sim.snapshot(&app.theme()).unwrap();
        assert!(snap
            .matches_image(fresh_path("classic-dark-graph"))
            .unwrap());
    }

    /// Stash node and label (M11-05): dashed square with the archive icon,
    /// "WIP on main" in the graph, STASHES and the details title; all three
    /// themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_stash_node() {
        use crate::settings::ThemeChoice;
        let mut app = crate::app::tests::graph_app();
        let _ = app.update(Message::CommitSelected("s".into()));
        for (choice, name) in [
            (ThemeChoice::Dark, "dark"),
            (ThemeChoice::Light, "light"),
            (ThemeChoice::ClassicDark, "classic-dark"),
        ] {
            app.settings.appearance.theme = choice;
            snapshot(&app, &format!("stash-node-{name}"));
        }
    }

    /// Preferences → Editor (M11-06) and a diff with a larger font and
    /// without line numbers.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_editor_settings() {
        let mut app = crate::app::tests::wide_diff_app(crate::settings::DiffMode::Hunk);
        let _ = app.update(Message::EditorFontSize(16));
        let _ = app.update(Message::EditorLineNumbersToggled(false));
        snapshot(&app, "editor-diff-16-no-numbers");
        let _ = app.update(Message::SettingsOpenedAt(crate::settings::Category::Editor));
        snapshot(&app, "editor-preferences");
    }

    /// Start screen like GitKraken's New Tab, both themes at 100 and 125 %.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_start_screen() {
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let mut app = start_app();
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            for percent in [100_u32, 125] {
                let scale = percent as f32 / 100.0;
                let _ = app.update(Message::SystemScaleChanged(scale));
                snapshot_scaled(&app, &format!("start-screen-{percent}-{theme}"), scale);
            }
        }
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
                MenuEntry::Item { label, .. } | MenuEntry::Check { label, .. } => {
                    Some(label.clone())
                }
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

    /// Dragging the border right of BRANCH / TAG with real mouse events
    /// resizes the column; a double click sets its optimal width (M11-02).
    /// Layout only, so part of every test run.
    #[test]
    fn graph_header_border_drags_and_fits() {
        use iced::mouse::{Button, Event as Mouse};
        use iced::{Event, Point};
        let mut app = crate::app::tests::graph_app();
        let messages = |app: &App, events: Vec<Event>, at: Point| -> Vec<Message> {
            let mut sim = simulator(app);
            sim.point_at(at);
            let _ = sim.simulate(events);
            sim.into_messages().collect()
        };
        let label = {
            let mut sim = simulator(&app);
            let label = sim
                .find("BRANCH / TAG")
                .unwrap_or_else(|e| panic!("no header: {e}"));
            iced_test::selector::Bounded::bounds(&label)
        };
        // The label starts after the cell padding; the handle lies over the
        // right edge of the 200 px cell.
        let border = Point::new(label.x - 4.0 + 200.0 - 2.0, label.center_y());
        let drag = vec![
            Event::Mouse(Mouse::ButtonPressed(Button::Left)),
            Event::Mouse(Mouse::CursorMoved {
                position: Point::new(border.x + 25.0, border.y),
            }),
            Event::Mouse(Mouse::CursorMoved {
                position: Point::new(border.x + 40.0, border.y + 300.0),
            }),
            Event::Mouse(Mouse::ButtonReleased(Button::Left)),
        ];
        for message in messages(&app, drag, border) {
            let _ = app.update(message);
        }
        assert_eq!(app.settings.appearance.graph_columns.refs, 240.0);
        assert!(app.column_drag.is_none());

        // Double click on the moved border: optimal width.
        let border = Point::new(border.x + 40.0, border.y);
        let click = [
            Event::Mouse(Mouse::ButtonPressed(Button::Left)),
            Event::Mouse(Mouse::ButtonReleased(Button::Left)),
        ];
        let double: Vec<Event> = click.iter().chain(click.iter()).cloned().collect();
        let produced = messages(&app, double, border);
        assert!(
            produced
                .iter()
                .any(|m| matches!(m, Message::ColumnFit { .. })),
            "{produced:?}"
        );
        for message in produced {
            let _ = app.update(message);
        }
        let refs = app.settings.appearance.graph_columns.refs;
        // The widest pill group: "feature".
        let pill = crate::overlay::text_width("feature", 12.0) + 14.0;
        assert!(refs > pill && refs < 200.0, "{refs}");
        // A press elsewhere in the header does not start a drag.
        let produced = messages(
            &app,
            vec![Event::Mouse(Mouse::ButtonPressed(Button::Left))],
            Point::new(label.center_x(), label.center_y()),
        );
        assert!(
            !produced
                .iter()
                .any(|m| matches!(m, Message::ColumnDragStarted { .. })),
            "{produced:?}"
        );
    }

    /// Header cells line up with the row cells (M11-03: the search and
    /// gear buttons lie over the header instead of taking room).
    /// Layout only, so part of every test run.
    #[test]
    fn graph_header_lines_up_with_the_rows() {
        let app = crate::app::tests::graph_app();
        let mut sim = simulator(&app);
        let x = |sim: &mut Simulator<'_, Message>, label: &str| {
            let target = sim
                .find(label)
                .unwrap_or_else(|e| panic!("no {label}: {e}"));
            iced_test::selector::Bounded::bounds(&target).x
        };
        let header = x(&mut sim, "AUTHOR");
        let row = x(&mut sim, "Bo Dev");
        assert!(
            (header - row).abs() < 0.5,
            "AUTHOR at {header}, rows at {row}"
        );
        let header = x(&mut sim, "COMMIT MESSAGE");
        let row = x(&mut sim, "Feature work");
        assert!((header - row).abs() < 0.5, "{header} vs {row}");
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

    /// Sections like GitKraken: GITFLOW, LOCAL, REMOTE, STASHES, TAGS,
    /// SUBMODULES (M9-06); the widget order is also the keyboard focus
    /// order. Layout only, part of every test run.
    #[test]
    fn left_panel_sections_are_ordered_like_gitkraken() {
        let app = crate::app::tests::flow_app();
        let mut sim = simulator(&app);
        let mut top = |label: &str| {
            let found = sim.find(label).unwrap_or_else(|e| panic!("{label}: {e}"));
            iced_test::selector::Bounded::bounds(&found).y
        };
        let order = [
            "GITFLOW",
            "LOCAL",
            "REMOTE",
            "STASHES",
            "TAGS",
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

    /// Resized graph columns (M11-02): a Graph column of one lane clips the
    /// lanes with a fade; wider Branch/Tag and Author columns; both themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_graph_columns() {
        let mut app = crate::app::tests::graph_app();
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            app.settings.appearance.graph_columns = crate::graph_columns::ColumnWidths {
                refs: 120.0,
                graph: Some(crate::ui::MIN_GRAPH_WIDTH),
                author: 180.0,
                ..Default::default()
            };
            snapshot(&app, &format!("graph-columns-narrow-{theme}"));
            app.settings.appearance.graph_columns.graph = Some(160.0);
            snapshot(&app, &format!("graph-columns-wide-{theme}"));
        }
    }

    /// Column menu at the gear (M11-03) with Date hidden, and a window too
    /// narrow for all columns: the table is clipped at the right.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_column_menu() {
        let mut app = crate::app::tests::graph_app();
        let _ = app.update(Message::GraphColumnShown(
            crate::graph_columns::ColumnId::Date,
            false,
        ));
        let _ = app.update(Message::GraphColumnMenu(iced::Point::new(1060.0, 88.0)));
        snapshot(&app, "graph-column-menu");
        let _ = app.update(Message::MenuClosed);
        let _ = app.update(Message::GraphColumnShown(
            crate::graph_columns::ColumnId::Date,
            true,
        ));
        snapshot_sized(&app, "graph-columns-narrow-window", (900.0, 600.0));
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

    /// Preferences → Identities with a default (M8-03), both themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_identities() {
        let mut app = ready_app(&["c"]);
        let _ = app.update(Message::GitDetected(git_core::GitAvailability::Available(
            git_core::MIN_GIT_VERSION,
        )));
        let _ = app.update(Message::SettingsOpenedAt(
            crate::settings::Category::Identities,
        ));
        for (name, email) in [
            ("Ana Lyst", "ana@example.com"),
            ("Ana Lyst", "ana@work.example"),
        ] {
            app.settings
                .identities
                .list
                .push(crate::settings::SavedIdentity {
                    name: name.into(),
                    email: email.into(),
                });
        }
        app.settings.identities.default = Some(0);
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            snapshot(&app, &format!("identities-{theme}"));
        }
        let _ = app.update(Message::IdentityAddStart);
        snapshot(&app, "identities-add");
    }

    /// Commit panel with and without an identity, and Preferences →
    /// Repository → Identity (M8-04), both themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_commit_identity() {
        let mut app = crate::app::tests::identity_commit_app(true);
        let mut none = crate::app::tests::identity_commit_app(false);
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let choice = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            app.settings.appearance.theme = choice;
            none.settings.appearance.theme = choice;
            snapshot(&app, &format!("commit-identity-{theme}"));
            snapshot(&none, &format!("commit-no-identity-{theme}"));
        }
        let _ = app.update(Message::IdentityMenu(iced::Point::new(1150.0, 640.0)));
        snapshot(&app, "commit-identity-menu");
        app.context_menu = None;
        let _ = app.update(Message::SettingsOpenedAt(
            crate::settings::Category::Identity,
        ));
        snapshot(&app, "preferences-repository-identity");
    }

    /// Preferences → Repository → SSH key with a chosen key (M8-06), both
    /// themes, and a custom command waiting for "Replace…".
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_ssh_key() {
        let mut app = crate::app::tests::ssh_app();
        crate::app::tests::set_ssh(
            &mut app,
            git_core::SshConfig {
                setting: git_core::SshSetting::Key("/home/me/.ssh/id_work".into()),
                origin: Some(git_core::ConfigOrigin::Local),
            },
        );
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::Light
            } else {
                crate::settings::ThemeChoice::Dark
            };
            snapshot(&app, &format!("ssh-key-{theme}"));
        }
        crate::app::tests::set_ssh(
            &mut app,
            git_core::SshConfig {
                setting: git_core::SshSetting::Custom("ssh -F ~/.ssh/work".into()),
                origin: Some(git_core::ConfigOrigin::Local),
            },
        );
        snapshot(&app, "ssh-key-custom");
    }

    /// An inherited core.sshCommand is shown with its origin (M8-06).
    #[test]
    fn inherited_ssh_command_shows_its_origin() {
        let mut app = crate::app::tests::ssh_app();
        crate::app::tests::set_ssh(
            &mut app,
            git_core::SshConfig {
                setting: git_core::SshSetting::Custom("ssh -v".into()),
                origin: Some(git_core::ConfigOrigin::Global),
            },
        );
        let mut sim = simulator(&app);
        let text = "Inherited from the global config: custom command ssh -v. Choosing a key \
                    overrides it for this repository.";
        assert!(sim.find(text).is_ok());
        assert!(sim
            .find("Inherited (from the global config) · current")
            .is_ok());
        assert!(sim.find("This repository has no SSH remotes").is_ok());
    }

    /// Toolbar without and with an external editor (M8-08).
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_toolbar_editor() {
        let mut app = ready_app(&["c"]);
        snapshot(&app, "toolbar-no-editor");
        app.settings.tools.editor_command = "code {path}".into();
        snapshot(&app, "toolbar-editor");
    }

    /// Toolbar buttons center icon and label (fixed width 64, spacing 2):
    /// labels of different length on adjacent buttons are exactly one
    /// button apart. Layout only, part of every test run.
    #[test]
    fn toolbar_labels_are_centered() {
        let app = ready_app(&["c"]);
        let mut sim = simulator(&app);
        let mut center = |label: &str| {
            let found = sim.find(label).unwrap_or_else(|e| panic!("{label}: {e}"));
            let bounds = iced_test::selector::Bounded::bounds(&found);
            bounds.x + bounds.width / 2.0
        };
        for (left, right) in [("Branch", "Stash"), ("Stash", "Pop"), ("Undo", "Redo")] {
            let distance = center(right) - center(left);
            assert!(
                (distance - 66.0).abs() < 0.5,
                "{left} → {right}: {distance} px between the label centers"
            );
        }
    }
}
