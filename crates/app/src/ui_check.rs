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
    snapshot_scaled_sized(app, name, scale, SIZE)
}

/// [`snapshot_scaled`] for a window of `window` logical pixels.
pub fn snapshot_scaled_sized(app: &App, name: &str, scale: f32, window: (f32, f32)) -> PathBuf {
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
    let size = iced::Size::new(window.0, window.1);
    let mut ui = UserInterface::build(app.view(), size, Cache::default(), &mut renderer);
    // Widgets such as buttons learn their status from the first redraw
    // event; without it they draw as disabled.
    let mut messages = Vec::new();
    let _ = ui.update(
        &[iced::Event::Window(iced::window::Event::RedrawRequested(
            std::time::Instant::now(),
        ))],
        iced::mouse::Cursor::Unavailable,
        &mut renderer,
        &mut iced::advanced::clipboard::Null,
        &mut messages,
    );
    ui.draw(
        &mut renderer,
        &theme,
        &iced::advanced::renderer::Style {
            text_color: base.text_color,
        },
        iced::mouse::Cursor::Unavailable,
    );
    let physical = iced::Size::new(
        (window.0 * scale).round() as u32,
        (window.1 * scale).round() as u32,
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
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
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
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
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

    /// Section headers (fix/sidebar-section-headers, finding #46): full
    /// text color, semibold, a hairline above every section (also the
    /// first) and the same space above and below the header; with and
    /// without GITFLOW, all collapsed, mixed and all expanded, both themes
    /// at 100 and 125 %.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_section_headers() {
        use crate::repo_view::Section;
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let choice = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            let mut flow = crate::app::tests::flow_app();
            flow.settings.appearance.theme = choice.clone();
            let mut plain = crate::app::tests::sidebar_app();
            plain.settings.appearance.theme = choice.clone();
            for (state, collapse) in [
                ("expanded", &[][..]),
                (
                    "mixed",
                    &[Section::Gitflow, Section::Remote, Section::Tags][..],
                ),
                ("collapsed", &ALL_SECTIONS[..]),
            ] {
                for (app, name) in [(&mut flow, "gitflow"), (&mut plain, "local-first")] {
                    set_collapsed(app, collapse);
                    for percent in [100_u32, 125] {
                        let scale = percent as f32 / 100.0;
                        let _ = app.update(Message::SystemScaleChanged(scale));
                        snapshot_scaled(
                            app,
                            &format!("section-headers-{name}-{state}-{theme}-{percent}"),
                            scale,
                        );
                    }
                }
            }
        }
    }

    const ALL_SECTIONS: [crate::repo_view::Section; 6] = [
        crate::repo_view::Section::Gitflow,
        crate::repo_view::Section::Local,
        crate::repo_view::Section::Remote,
        crate::repo_view::Section::Stashes,
        crate::repo_view::Section::Tags,
        crate::repo_view::Section::Submodules,
    ];

    /// Collapses exactly the sections `collapsed` of the active tab.
    fn set_collapsed(app: &mut App, collapsed: &[crate::repo_view::Section]) {
        for section in ALL_SECTIONS {
            let is = app.tabs[0].view().unwrap().collapsed.contains(&section);
            if is != collapsed.contains(&section) {
                let _ = app.update(Message::SectionToggled(section));
            }
        }
    }

    /// Bounds of every hairline in the simulated view, top to bottom.
    fn hairlines(sim: &mut Simulator<'_, Message>) -> Vec<iced::Rectangle> {
        use iced_test::selector::Candidate;
        let mut out = Vec::new();
        let _ = sim.find(|candidate: Candidate<'_>| -> Option<()> {
            if let Candidate::Custom { bounds, state, .. } = candidate {
                if state.is::<crate::icons::Hairline>() {
                    out.push(bounds);
                }
            }
            None
        });
        out.sort_by(|a, b| a.y.total_cmp(&b.y));
        out
    }

    /// Bounds of the section header (its button) showing `title` in a
    /// left panel `sidebar` large: the lowest of the widest containers
    /// around the label inside the panel's padding.
    fn header_bounds(
        sim: &mut Simulator<'_, Message>,
        sidebar: iced::Rectangle,
        title: &str,
    ) -> Option<iced::Rectangle> {
        use iced_test::selector::Candidate;
        let label = iced_test::selector::Bounded::bounds(&sim.find(title).ok()?);
        let mut around = Vec::new();
        let _ = sim.find(|candidate: Candidate<'_>| -> Option<()> {
            if let Candidate::Container { bounds, .. } = candidate {
                let contains = bounds.x <= label.x
                    && bounds.y <= label.y
                    && bounds.x + bounds.width >= label.x + label.width
                    && bounds.y + bounds.height >= label.y + label.height;
                if contains && bounds.width < sidebar.width {
                    around.push(bounds);
                }
            }
            None
        });
        let widest = around.iter().map(|b| b.width).fold(0.0, f32::max);
        around
            .into_iter()
            .filter(|b| b.width == widest)
            .min_by(|a, b| a.height.total_cmp(&b.height))
    }

    /// Finding #46: every visible section of the left panel starts with a
    /// hairline, also the first (with and without GITFLOW, with the
    /// "Viewing x/y" line); a collapsed header has `SECTION_PAD` above and
    /// below it, so it sits centered between two lines. Layout only.
    #[test]
    fn section_headers_sit_between_their_lines() {
        let titles = [
            "GITFLOW",
            "LOCAL",
            "REMOTE",
            "STASHES",
            "TAGS",
            "SUBMODULES",
        ];
        let mut plain = ready_app(&["c"]);
        let _ = plain.update(Message::GitDetected(git_core::GitAvailability::Available(
            git_core::MIN_GIT_VERSION,
        )));
        for (name, mut app, first, viewing) in [
            ("gitflow", crate::app::tests::flow_app(), "GITFLOW", false),
            ("viewing", crate::app::tests::sidebar_app(), "LOCAL", true),
            ("plain", plain, "LOCAL", false),
        ] {
            set_collapsed(&mut app, &ALL_SECTIONS);
            let mut sim = simulator(&app);
            let sidebar = pane_bounds(&mut sim, crate::panes::PaneKind::Sidebar);
            let lines: Vec<_> = hairlines(&mut sim)
                .into_iter()
                .filter(|l| l.x < sidebar.x + sidebar.width)
                .collect();
            let headers: Vec<(&str, iced::Rectangle)> = titles
                .iter()
                .filter_map(|t| header_bounds(&mut sim, sidebar, t).map(|b| (*t, b)))
                .collect();
            assert_eq!(headers[0].0, first, "{name}");
            assert_eq!(viewing_shown(&mut sim), viewing, "{name}");
            assert_eq!(lines.len(), headers.len(), "{name}: one line per section");
            // The first line sits right below the filter field.
            let top = lines[0].y - sidebar.y;
            assert!((30.0..80.0).contains(&top), "{name}: first line at {top}");
            let pad = crate::ui::sidebar::SECTION_PAD;
            for (i, (title, header)) in headers.iter().enumerate() {
                let above = header.y - (lines[i].y + lines[i].height);
                assert!((above - pad).abs() < 0.01, "{name} {title}: {above} above");
                if let Some(next) = lines.get(i + 1) {
                    let below = next.y - (header.y + header.height);
                    assert!((below - pad).abs() < 0.01, "{name} {title}: {below} below");
                }
            }
        }
    }

    /// Whether the "Viewing x/y" line of the left panel is shown.
    fn viewing_shown(sim: &mut Simulator<'_, Message>) -> bool {
        use iced_test::selector::Candidate;
        sim.find(|candidate: Candidate<'_>| match candidate {
            Candidate::Text { content, .. } if content.starts_with("Viewing") => Some(()),
            _ => None,
        })
        .is_ok()
    }

    /// Tree order, tag folders and the checked-out marker (M11-11): main
    /// checked out at depth 0, then a branch inside a folder; branches
    /// before folders; TAGS with a folder; both themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_tree_order() {
        use git_core::RefKind::Tag;
        let mut app = crate::app::tests::sidebar_app();
        let tag = |name: &str| git_core::Reference {
            name: name.into(),
            kind: Tag,
            target: "c".into(),
            is_head: false,
            upstream: None,
            ahead_behind: None,
        };
        let view = app.tabs[0].view_mut().unwrap();
        let mut snap = (*view.snapshot).clone();
        snap.refs
            .extend([tag("release/1.0"), tag("release/2.0"), tag("v0.9")]);
        view.snapshot = std::sync::Arc::new(snap);
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            snapshot(&app, &format!("tree-order-head-top-{theme}"));
        }
        // The checked-out branch inside a folder.
        let view = app.tabs[0].view_mut().unwrap();
        let mut snap = (*view.snapshot).clone();
        for reference in &mut snap.refs {
            reference.is_head = reference.name == "feature/ui/diff-modes";
        }
        view.snapshot = std::sync::Arc::new(snap);
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            snapshot(&app, &format!("tree-order-head-in-folder-{theme}"));
        }
    }

    /// Diff header icons (M15-08): each sends its message. Layout only, so
    /// part of every test run.
    #[test]
    fn diff_header_icons_send_their_messages() {
        let app = crate::app::tests::app_with_open_diff(git_core::DiffKind::Commit("c".into()));
        let title = {
            let mut sim = simulator(&app);
            let found = sim.find("a").unwrap_or_else(|e| panic!("no path: {e}"));
            iced_test::selector::Bounded::bounds(&found)
        };
        let messages = clicks_along(&app, title.x, title.center_y(), SIZE.0 - title.x);
        let has = |pred: &dyn Fn(&Message) -> bool| messages.iter().any(pred);
        assert!(has(&|m| matches!(m,
            Message::FileBlameOpened { path, start: Some(s), rev: Some(r) }
                if path == "a" && s == "c" && r == "c")));
        assert!(has(&|m| matches!(m,
            Message::FileHistoryOpened { path, start: Some(s) } if path == "a" && s == "c")));
        for mode in crate::settings::DiffMode::ALL {
            assert!(
                has(&|m| matches!(m, Message::DiffModeChanged(x) if *x == mode)),
                "{mode:?}"
            );
        }
        assert!(has(&|m| matches!(m, Message::ExternalDiff)));
        assert!(has(&|m| matches!(m, Message::DiffClosed)));
    }

    /// Messages of a press at `from`, a move to `to` and a release.
    fn drag(app: &App, from: iced::Point, to: iced::Point) -> Vec<Message> {
        use iced::mouse::{Button, Event as Mouse};
        use iced::Event;
        let mut sim = simulator(app);
        sim.point_at(from);
        let _ = sim.simulate(vec![
            Event::Mouse(Mouse::CursorMoved { position: from }),
            Event::Mouse(Mouse::ButtonPressed(Button::Left)),
            Event::Mouse(Mouse::CursorMoved { position: to }),
            Event::Mouse(Mouse::ButtonReleased(Button::Left)),
        ]);
        sim.into_messages().collect()
    }

    /// The diff minimap (M15-09): a click centers the view on that
    /// position, dragging follows, the wheel scrolls. Inline and Split
    /// only. Layout only, so part of every test run.
    #[test]
    fn diff_minimap_click_drag_and_wheel() {
        use crate::settings::DiffMode;
        let seeks = |messages: &[Message]| -> Vec<f32> {
            messages
                .iter()
                .filter_map(|m| match m {
                    Message::DiffMinimapSeek(at) => Some(*at),
                    _ => None,
                })
                .collect()
        };
        for mode in [DiffMode::Inline, DiffMode::Split, DiffMode::Hunk] {
            let mut app = crate::app::tests::mode_app(mode, 400, 20, 380);
            let _ = app.update(Message::DiffScrolled {
                offset: 0.0,
                height: 500.0,
                width: 900.0,
            });
            // The strip: scan the middle line of the diff for it.
            let y = SIZE.1 / 2.0;
            let mut x = SIZE.0 / 2.0;
            let mut found = None;
            while x < SIZE.0 && found.is_none() {
                let at = iced::Point::new(x, y);
                if !seeks(&drag(&app, at, at)).is_empty() {
                    found = Some(x);
                }
                x += 2.0;
            }
            if mode == DiffMode::Hunk {
                assert_eq!(found, None, "no minimap in the Hunk view");
                continue;
            }
            let x = found.unwrap_or_else(|| panic!("no minimap in {mode:?}"));
            // Drag from the upper to the lower part of the strip.
            let messages = drag(
                &app,
                iced::Point::new(x, y - 100.0),
                iced::Point::new(x, y + 100.0),
            );
            let seeks = seeks(&messages);
            assert_eq!(seeks.len(), 2, "{mode:?}: {messages:?}");
            assert!(seeks[1] > seeks[0], "{seeks:?}");
            // The click centers the view there.
            let _ = app.update(Message::DiffMinimapSeek(0.5));
            let diff = crate::app::tests::open_diff(&app);
            let content = diff.content_height();
            assert!((diff.scroll.offset - (content / 2.0 - 250.0)).abs() < 0.5);
            // The wheel scrolls by pixels, clamped.
            let before = diff.scroll.offset;
            let _ = app.update(Message::DiffMinimapWheel(120.0));
            assert_eq!(
                crate::app::tests::open_diff(&app).scroll.offset,
                before + 120.0
            );
            let _ = app.update(Message::DiffMinimapWheel(-1e9));
            assert_eq!(crate::app::tests::open_diff(&app).scroll.offset, 0.0);
            // The wheel over the strip asks for it.
            let mut sim = simulator(&app);
            let at = iced::Point::new(x, y);
            sim.point_at(at);
            let _ = sim.simulate(vec![iced::Event::Mouse(
                iced::mouse::Event::WheelScrolled {
                    delta: iced::mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
                },
            )]);
            let messages: Vec<Message> = sim.into_messages().collect();
            assert!(
                messages
                    .iter()
                    .any(|m| matches!(m, Message::DiffMinimapWheel(p) if *p == 60.0)),
                "{messages:?}"
            );
        }
    }

    /// Minimap snapshots (M15-09): a long file with changes at the top, in
    /// the middle and at the end, Inline and Split, both themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_diff_minimap() {
        use crate::settings::{DiffMode, ThemeChoice};
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            for mode in [DiffMode::Inline, DiffMode::Split] {
                let mut app = crate::app::tests::minimap_app(mode);
                app.settings.appearance.theme = choice.clone();
                let name = mode.label().to_lowercase();
                snapshot(&app, &format!("diff-minimap-{name}-{theme}"));
            }
        }
    }

    /// Diff header in Hunk, Inline and Split (M15-08), both themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_diff_header_icons() {
        use crate::settings::{DiffMode, ThemeChoice};
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            for mode in DiffMode::ALL {
                let mut app =
                    crate::app::tests::app_with_open_diff(git_core::DiffKind::Commit("c".into()));
                app.settings.appearance.theme = choice.clone();
                app.settings.diff.view_mode = mode;
                let name = mode.label().to_lowercase();
                snapshot_sized(
                    &app,
                    &format!("diff-header-{name}-{theme}"),
                    (1200.0, 300.0),
                );
            }
        }
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_diff_mode_toggle() {
        let mut app = crate::app::tests::app_with_open_diff(git_core::DiffKind::Unstaged);
        snapshot(&app, "diff-mode-0-hunk");
        let _ = app.update(Message::DiffModeChanged(crate::settings::DiffMode::Split));
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
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
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
                ThemeChoice::theme("light")
            } else {
                ThemeChoice::theme("dark")
            };
            app.settings.appearance.theme = choice.clone();
            added.settings.appearance.theme = choice.clone();
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
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
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
        // M15-04: initialized, with the experimental layout switch.
        let mut app = crate::app::tests::flow_app();
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            let _ = app.update(Message::SettingsOpenedAt(
                crate::settings::Category::Gitflow,
            ));
            snapshot(&app, &format!("gitflow-preferences-initialized-{theme}"));
        }
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_gitflow_section() {
        let mut app = crate::app::tests::flow_app();
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            snapshot(&app, &format!("gitflow-section-{theme}"));
            // M15-04: the hovered feature row with its finish flag.
            let path = fresh_path(&format!("gitflow-section-hover-{theme}"));
            let mut sim = simulator(&app);
            let row = sim.find("login").unwrap();
            let at = iced_test::selector::Bounded::bounds(&row).center();
            sim.point_at(at);
            let _ = sim.simulate(vec![iced::Event::Mouse(iced::mouse::Event::CursorMoved {
                position: at,
            })]);
            let snap = sim.snapshot(&app.theme()).unwrap();
            assert!(snap.matches_image(&path).unwrap());
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
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            let mut app = stash_app();
            app.settings.appearance.theme = choice.clone();
            pop_stash(&mut app, git_core::StashOutcome::Conflicts);
            with_conflicts(&mut app, &["a.txt"]);
            snapshot(&app, &format!("stash-conflict-0-{theme}"));
            resolve_all(&mut app);
            snapshot(&app, &format!("stash-conflict-1-resolved-{theme}"));
            let mut orphaned = staging_app(&[], &[]);
            orphaned.settings.appearance.theme = choice.clone();
            with_conflicts(&mut orphaned, &["a.txt"]);
            snapshot(&orphaned, &format!("stash-conflict-2-orphaned-{theme}"));
        }
    }

    /// Conflict view (M12-04): "conflict 2 of 3" with arrows, side headers
    /// "Commit <sha> on <branch>", both themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_conflict_navigation() {
        use git_core::{Chunk, ConflictHunk, Head, RefKind, Reference, RepoState};
        let (main, topic) = ("1a2b3c4d".repeat(5), "5e6f7a8b".repeat(5));
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let mut app = crate::app::tests::app_in_state(RepoState::Merging {
                merge_head: topic.clone(),
            });
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            let view = app.tabs[0].view_mut().unwrap();
            let mut repo = (*view.snapshot).clone();
            repo.head = Head::Branch {
                name: "main".into(),
                target: main.clone(),
            };
            repo.refs = vec![
                Reference {
                    name: "main".into(),
                    kind: RefKind::LocalBranch,
                    target: main.clone(),
                    is_head: true,
                    upstream: None,
                    ahead_behind: None,
                },
                Reference {
                    name: "topic".into(),
                    kind: RefKind::LocalBranch,
                    target: topic.clone(),
                    is_head: false,
                    upstream: None,
                    ahead_behind: None,
                },
            ];
            view.apply_snapshot(std::sync::Arc::new(repo));
            crate::app::tests::with_conflicts(&mut app, &["src/lib.rs"]);
            let tab = app.tabs[0].id;
            let _ = app.update(Message::ConflictOpened("src/lib.rs".into()));
            let hunk = |n: &str| {
                Chunk::Conflict(ConflictHunk {
                    ours: vec![format!("    let value = {n};\n")],
                    base: vec![],
                    theirs: vec![format!("    let value = {n}0;\n")],
                })
            };
            let clean = |line: &str| Chunk::Clean(vec![format!("{line}\n")]);
            let loaded = git_core::LoadedConflict {
                path: "src/lib.rs".into(),
                stages: git_core::ConflictStages::default(),
                encoding: git_core::ConflictEncoding::Utf8,
                content: git_core::ConflictContent::Text(git_core::ConflictText {
                    chunks: vec![
                        clean("fn one() {"),
                        hunk("1"),
                        clean("}"),
                        clean("fn two() {"),
                        hunk("2"),
                        clean("}"),
                        hunk("3"),
                    ],
                    eol: "\n",
                }),
            };
            let _ = app.update(Message::ConflictLoaded {
                tab,
                path: "src/lib.rs".into(),
                result: Ok(loaded.clone()),
            });
            let _ = app.update(Message::ConflictStep { next: true });
            snapshot(&app, &format!("conflict-navigation-{theme}"));
            // M15-01: a UTF-16 conflict shows its encoding next to the path.
            let _ = app.update(Message::ConflictLoaded {
                tab,
                path: "src/lib.rs".into(),
                result: Ok(git_core::LoadedConflict {
                    encoding: git_core::ConflictEncoding::Utf16Le,
                    ..loaded
                }),
            });
            snapshot(&app, &format!("conflict-utf16-{theme}"));
        }
    }

    /// Operation state like GitKraken (M12-05): graph row, panel title,
    /// Conflicted/Resolved Files with "Mark all resolved", "Commit and
    /// merge"/"Abort merge"; then a commit selected with the details
    /// header; a rebase without conflicts. All themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_operation_state() {
        use crate::settings::ThemeChoice;
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            let mut app = crate::app::tests::app_in_state(git_core::RepoState::Merging {
                merge_head: "x".into(),
            });
            app.settings.appearance.theme = choice.clone();
            let view = app.tabs[0].view_mut().unwrap();
            let tree = git_core::WorkingTree {
                unstaged: vec![git_core::FileChange {
                    path: "src/main.rs".into(),
                    kind: git_core::ChangeKind::Conflicted,
                    old_path: None,
                }],
                staged: ["README.md", "src/lib.rs"]
                    .iter()
                    .map(|p| git_core::FileChange {
                        path: (*p).into(),
                        kind: git_core::ChangeKind::Modified,
                        old_path: None,
                    })
                    .collect(),
            };
            view.operation_conflicts =
                Some(["src/lib.rs".to_owned(), "src/main.rs".to_owned()].into());
            view.snapshot = std::sync::Arc::new(view.snapshot.with_working_tree(tree).unwrap());
            let _ = app.update(Message::WipSelected);
            snapshot(&app, &format!("operation-0-merge-{theme}"));
            let id = app.tabs[0].view().unwrap().snapshot.commits[0].id.clone();
            let _ = app.update(Message::CommitSelected(id));
            snapshot(&app, &format!("operation-1-details-{theme}"));

            let mut rebase = crate::app::tests::app_in_state(git_core::RepoState::Rebasing {
                branch: Some("topic".into()),
                onto: None,
                step: Some((2, 3)),
                commit: None,
            });
            rebase.settings.appearance.theme = choice.clone();
            let _ = rebase.update(Message::WipSelected);
            snapshot(&rebase, &format!("operation-2-rebase-{theme}"));
        }
    }

    /// SUBMODULES section (M12-06): every state, a nested submodule, the
    /// row menu and the out-of-date toast; dark and light.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_submodules() {
        use git_core::submodule::{Submodule, SubmoduleState as S};
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let mut app = crate::app::tests::staging_app(&[], &[]);
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            let tab = app.tabs[0].id;
            let list = [
                ("libs/core", S::Current),
                ("libs/core/nested", S::Modified),
                ("libs/ui", S::OutOfDate),
                ("vendor/zlib", S::NotInitialized),
                ("tools/gen", S::Conflict),
            ]
            .into_iter()
            .map(|(path, state)| Submodule {
                path: path.into(),
                commit: "a".repeat(40),
                state,
            })
            .collect::<Vec<_>>();
            for head in ["h1", "h2"] {
                let _ = app.update(Message::SubmodulesLoaded {
                    tab,
                    head: Some(head.into()),
                    result: Ok(list.clone()),
                });
            }
            let _ = app.update(Message::SubmoduleContextMenu {
                path: "libs/ui".into(),
                position: iced::Point::new(120.0, 420.0),
            });
            snapshot(&app, &format!("submodules-{theme}"));
        }
    }

    /// STASHES as a tree (M12-10): stashes in stash order, then folders;
    /// "WIP on feature/x" not split; one folder collapsed; the rename
    /// dialog. Dark and light.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_stash_folders() {
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let mut app = crate::app::tests::stash_app();
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            let view = app.tabs[0].view_mut().unwrap();
            let mut repo = (*view.snapshot).clone();
            let template = repo.stashes[0].clone();
            repo.stashes = [
                "WIP on feature/login: 1a2b3c4 Add form",
                "On main: experiments/fast path",
                "On main: quick fix",
                "On main: experiments/cache",
                "On main: archive/2025/old idea",
            ]
            .iter()
            .enumerate()
            .map(|(index, message)| git_core::Stash {
                index,
                message: (*message).into(),
                id: format!("s{index}"),
                ..template.clone()
            })
            .collect();
            view.snapshot = std::sync::Arc::new(repo);
            view.folders_collapsed
                .insert(crate::branch_tree::stash_folder_key("archive"));
            snapshot(&app, &format!("stash-folders-{theme}"));
            let _ = app.update(Message::RenameStashStart("s2".into()));
            snapshot(&app, &format!("stash-rename-{theme}"));
        }
    }

    /// Multi-selection in the left panel (M12-11): selected rows, the bulk
    /// menu and the grouped dialog; dark and light.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_bulk_delete() {
        use git_core::RefKind::{LocalBranch, RemoteBranch, Tag};
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let mut app = crate::app::tests::sidebar_app();
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            for (kind, name, toggle) in [
                (LocalBranch, "main", false),
                (LocalBranch, "feature/ui/diff-modes", true),
                (RemoteBranch, "origin/develop", true),
                (Tag, "v1.0.0-rc.2", true),
            ] {
                let _ = app.update(Message::ModifiersChanged(if toggle {
                    iced::keyboard::Modifiers::COMMAND
                } else {
                    iced::keyboard::Modifiers::empty()
                }));
                let _ = app.update(Message::RefClicked {
                    name: name.into(),
                    kind,
                });
            }
            let _ = app.update(Message::ModifiersChanged(iced::keyboard::Modifiers::empty()));
            let _ = app.update(Message::BranchContextMenu {
                name: "origin/develop".into(),
                kind: RemoteBranch,
                position: iced::Point::new(150.0, 330.0),
            });
            snapshot(&app, &format!("bulk-0-menu-{theme}"));
            app.context_menu = None;
            let _ = app.update(Message::BulkDeleteStart);
            snapshot(&app, &format!("bulk-1-dialog-{theme}"));
        }
    }

    /// Palette settings entries and a deep link (M12-12): "toggle"
    /// filtered, then Preferences → General with "Default branch name"
    /// highlighted; dark and light.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_palette_settings() {
        use crate::shortcuts::Shortcut;
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let mut app = crate::app::tests::staging_app(&[], &[]);
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            let _ = app.update(crate::app::tests::checked(Shortcut::CommandPalette, false));
            let _ = app.update(Message::PaletteQuery("toggle".into()));
            snapshot(&app, &format!("palette-settings-0-{theme}"));
            let _ = app.update(Message::PaletteQuery("Preferences: Default branch".into()));
            let _ = app.update(crate::app::tests::checked(Shortcut::Confirm, true));
            snapshot(&app, &format!("palette-settings-1-{theme}"));
        }
    }

    /// M12-14: the trust screen of a repository Git refuses, and the start
    /// screen with a WSL badge in the recent list; dark and light.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_trust_and_wsl() {
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let choice = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            let mut app = crate::app::tests::staging_app(&[], &[]);
            app.settings.appearance.theme = choice.clone();
            app.tabs[0].state = crate::app::TabState::Failed {
                path: "/srv/shared/project".into(),
                error: "fatal: detected dubious ownership in repository at '/srv/shared/project'"
                    .into(),
            };
            snapshot(&app, &format!("trust-{theme}"));
            let (mut start, _) = App::new(Vec::new(), None);
            start.settings.appearance.theme = choice.clone();
            start.session.touch_recent(
                std::path::Path::new(r"\\wsl.localhost\Ubuntu\home\me\gitloom"),
                2,
            );
            start
                .session
                .touch_recent(std::path::Path::new("/home/me/local-repo"), 1);
            snapshot(&start, &format!("start-wsl-badge-{theme}"));
        }
    }

    /// Git flow layout (M13-02) of the classic diagram: production left,
    /// hotfix, release, development, features right; Dark and Light.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_flow_layout() {
        let (_repo, snapshot) = crate::snapshot::flow_tests::classic_snapshot();
        for (light, theme) in [(false, "dark"), (true, "light")] {
            let mut app = crate::app::tests::graph_app();
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            for (on, name) in [(false, "default"), (true, "flow")] {
                let view = app.tabs[0].view_mut().unwrap();
                view.snapshot = std::sync::Arc::new(snapshot.with_flow_layout(on).unwrap());
                snapshot_sized(
                    &app,
                    &format!("flow-layout-{name}-{theme}"),
                    (1200.0, 520.0),
                );
            }
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
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
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
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
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

    /// `ready_app` whose commits carry real messages and authors (M14).
    pub(crate) fn details_app(messages: &[(&str, &str, &str)]) -> App {
        let ids: Vec<&str> = messages.iter().map(|(id, _, _)| *id).collect();
        let mut app = ready_app(&ids);
        let _ = app.update(Message::GitDetected(git_core::GitAvailability::Available(
            git_core::MIN_GIT_VERSION,
        )));
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        let mut commits = (*snapshot.commits).clone();
        for (commit, (_, summary, body)) in commits.iter_mut().zip(messages) {
            commit.summary = (*summary).into();
            commit.body = (*body).into();
            commit.author = "Ana Lyst".into();
            commit.email = "ana@example.com".into();
        }
        snapshot.commits = std::sync::Arc::new(commits);
        view.snapshot = std::sync::Arc::new(snapshot);
        app
    }

    /// Author block with and without avatars, co-authors and a merge
    /// commit's two parents (M14-02).
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_author_block() {
        let body = "Pair work.\n\nCo-authored-by: Bo Kim <bo@example.com>\nCo-authored-by: Cy Dee <cy@example.com>";
        let mut app = details_app(&[
            ("c3", "Merge branch 'topic'", ""),
            ("c2", "Add the author block", body),
            ("c1", "Initial commit", ""),
        ]);
        // c3 merges c1 as second parent; c2 has a different committer.
        {
            let view = app.tabs[0].view_mut().unwrap();
            let mut snapshot = (*view.snapshot).clone();
            let mut commits = (*snapshot.commits).clone();
            commits[0].parents.push("c1".into());
            commits[1].committer = "Release Bot".into();
            snapshot.commits = std::sync::Arc::new(commits);
            view.snapshot = std::sync::Arc::new(snapshot);
        }
        for (id, name) in [("c2", "co-authors"), ("c3", "merge")] {
            let _ = app.update(Message::CommitSelected(id.into()));
            snapshot(&app, &format!("author-block-{name}-initials"));
        }
        // Avatars on: the app icon stands in for a Gravatar image.
        let icon = include_bytes!("../assets/app-icon/icon-64.png").to_vec();
        for email in ["ana@example.com", "bo@example.com"] {
            let hash = crate::avatars::gravatar_hash(email);
            let _ = app.avatars.request([email]);
            let _ = app
                .avatars
                .finished(&hash, Ok(crate::avatars::Cached::Image(icon.clone())));
        }
        let _ = app.update(Message::CommitSelected("c2".into()));
        snapshot(&app, "author-block-co-authors-avatars");
    }

    /// `details_app` with commit c2 selected, its changes and all files.
    fn file_list_app() -> App {
        let mut app = details_app(&[("c2", "Rework the panel", ""), ("c1", "Initial", "")]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::CommitSelected("c2".into()));
        let change = |path: &str, kind| git_core::FileChange {
            path: path.into(),
            kind,
            old_path: None,
        };
        let _ = app.update(Message::CommitChangesLoaded {
            tab,
            id: "c2".into(),
            result: Ok(crate::repo_view::DetailChanges::Commit(vec![
                change("src/ui/panel.rs", git_core::ChangeKind::Modified),
                change("src/ui/old.rs", git_core::ChangeKind::Deleted),
                change("docs/guide.md", git_core::ChangeKind::Added),
                change("Cargo.toml", git_core::ChangeKind::Modified),
            ])),
        });
        app
    }

    /// Finding #42: one Expand/Collapse button in the first row of the
    /// frame whose label follows the folders; hidden in Path mode and
    /// without folders. Layout only.
    #[test]
    fn expand_collapse_toggle_follows_the_folders() {
        let mut app = file_list_app();
        let has = |app: &App, label: &str| simulator(app).find(label).is_ok();
        assert!(
            !has(&app, "Collapse all") && !has(&app, "Expand all"),
            "path mode"
        );
        let _ = app.update(Message::FileListTreeChosen(true));
        assert!(has(&app, "Collapse all") && !has(&app, "Expand all"));
        click(&mut app, "Collapse all");
        assert_eq!(app.tabs[0].view().unwrap().file_tree_collapsed.len(), 3);
        assert!(has(&app, "Expand all") && !has(&app, "Collapse all"));
        click(&mut app, "Expand all");
        assert!(app.tabs[0].view().unwrap().file_tree_collapsed.is_empty());
        assert!(has(&app, "Collapse all"));
        // Collapsing one folder by hand flips the label back.
        let _ = app.update(Message::FileListFolderToggled("src/ui".into()));
        assert!(has(&app, "Expand all") && !has(&app, "Collapse all"));
        // A commit without folders hides the button.
        let tab = app.tabs[0].id;
        let _ = app.update(Message::CommitChangesLoaded {
            tab,
            id: "c2".into(),
            result: Ok(crate::repo_view::DetailChanges::Commit(vec![
                git_core::FileChange {
                    path: "Cargo.toml".into(),
                    kind: git_core::ChangeKind::Modified,
                    old_path: None,
                },
            ])),
        });
        assert!(
            !has(&app, "Collapse all") && !has(&app, "Expand all"),
            "no folders"
        );
    }

    /// Finding #42: the commit details do not scroll; the file list has its
    /// own frame with a scrollbar, the message box at most a fifth of the
    /// panel. Long and short message, Path and Tree, a stash; Dark and Light.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_details_layout() {
        use crate::settings::ThemeChoice;
        let long_body = (1..=40)
            .map(|i| format!("Line {i} of a long description that wraps in the panel."))
            .collect::<Vec<_>>()
            .join("\n");
        let files: Vec<git_core::FileChange> = (0..60)
            .map(|i| git_core::FileChange {
                path: format!("src/module{}/file{i:02}.rs", i % 4),
                kind: if i % 5 == 0 {
                    git_core::ChangeKind::Added
                } else {
                    git_core::ChangeKind::Modified
                },
                old_path: None,
            })
            .collect();
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            let mut app = details_app(&[
                ("c3", "Fix the graph header", ""),
                ("c2", "Add the message box", &long_body),
                ("c1", "Initial commit", ""),
            ]);
            app.settings.appearance.theme = choice.clone();
            let tab = app.tabs[0].id;
            for (id, name) in [("c2", "long"), ("c3", "short")] {
                let _ = app.update(Message::CommitSelected(id.into()));
                let _ = app.update(Message::CommitChangesLoaded {
                    tab,
                    id: id.into(),
                    result: Ok(crate::repo_view::DetailChanges::Commit(files.clone())),
                });
                let _ = app.update(Message::FileListTreeChosen(false));
                snapshot(&app, &format!("details-layout-{name}-path-{theme}"));
                let _ = app.update(Message::FileListTreeChosen(true));
                snapshot(&app, &format!("details-layout-{name}-tree-{theme}"));
            }
            let _ = app.update(Message::FileListTreeChosen(false));
            // A stash: its sections inside the frame.
            let mut app = crate::app::tests::graph_app();
            app.settings.appearance.theme = choice.clone();
            let tab = app.tabs[0].id;
            let _ = app.update(Message::CommitSelected("s".into()));
            let _ = app.update(Message::CommitChangesLoaded {
                tab,
                id: "s".into(),
                result: Ok(crate::repo_view::DetailChanges::Stash(
                    git_core::StashChanges {
                        staged: files[..3].to_vec(),
                        unstaged: files[3..30].to_vec(),
                        untracked: files[30..33].to_vec(),
                    },
                )),
            });
            snapshot(&app, &format!("details-layout-stash-{theme}"));
        }
    }

    /// Unchanged files of "View all files" are muted and open the file's
    /// content at the commit (M14-03). Layout only.
    #[test]
    fn unchanged_files_open_their_content() {
        let mut app = file_list_app();
        let tab = app.tabs[0].id;
        let _ = app.update(Message::FileListAllFilesToggled(true));
        let _ = app.update(Message::AllFilesLoaded {
            tab,
            id: "c2".into(),
            result: Ok(vec![
                "Cargo.toml".into(),
                "LICENSE".into(),
                "docs/guide.md".into(),
                "src/ui/panel.rs".into(),
            ]),
        });
        let messages: Vec<Message> = {
            let mut sim = simulator(&app);
            sim.click("LICENSE")
                .unwrap_or_else(|e| panic!("no row: {e}"));
            sim.into_messages().collect()
        };
        assert!(
            messages.iter().any(|m| matches!(m,
                Message::FileBlameOpened { path, rev: Some(rev), .. }
                    if path == "LICENSE" && rev == "c2")),
            "{messages:?}"
        );
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_file_list() {
        let mut app = file_list_app();
        let tab = app.tabs[0].id;
        snapshot(&app, "file-list-path");
        let _ = app.update(Message::FileListTreeChosen(true));
        snapshot(&app, "file-list-tree");
        let _ = app.update(Message::FileListFolderToggled("src/ui".into()));
        let _ = app.update(Message::FileListSortToggled);
        snapshot(&app, "file-list-tree-collapsed-desc");
        let _ = app.update(Message::FileListSortToggled);
        let _ = app.update(Message::FileListFolderToggled("src/ui".into()));
        let _ = app.update(Message::FileListAllFilesToggled(true));
        let _ = app.update(Message::AllFilesLoaded {
            tab,
            id: "c2".into(),
            result: Ok(vec![
                "Cargo.toml".into(),
                "LICENSE".into(),
                "docs/guide.md".into(),
                "src/main.rs".into(),
                "src/ui/panel.rs".into(),
            ]),
        });
        snapshot(&app, "file-list-tree-all");
        app.settings.appearance.theme = crate::settings::ThemeChoice::theme("light");
        let _ = app.update(Message::FileListTreeChosen(false));
        snapshot(&app, "file-list-path-all-light");
    }

    /// Messages of hovering and clicking every few pixels along the line
    /// `y` from `x` for `length` pixels.
    fn clicks_along(app: &App, x: f32, y: f32, length: f32) -> Vec<Message> {
        use iced::mouse::{Button, Event as Mouse};
        use iced::{Event, Point};
        let mut messages = Vec::new();
        let mut at_x = x;
        while at_x < x + length {
            let at = Point::new(at_x, y);
            let mut sim = simulator(app);
            sim.point_at(at);
            let _ = sim.simulate(vec![
                Event::Mouse(Mouse::CursorMoved { position: at }),
                Event::Mouse(Mouse::ButtonPressed(Button::Left)),
                Event::Mouse(Mouse::ButtonReleased(Button::Left)),
            ]);
            messages.extend(sim.into_messages());
            at_x += 3.0;
        }
        messages
    }

    /// The message box (M15-06): a click on the text does not open the
    /// modal; the button at the right of the header row does (finding
    /// #42, #43), also when editing is blocked (then read-only). Layout
    /// only, so part of every test run.
    #[test]
    fn header_button_opens_the_message_modal() {
        let mut app = details_app(&[("c2", "Fix the typo", "Body text"), ("c1", "Initial", "")]);
        let _ = app.update(Message::CommitSelected("c2".into()));
        let tab = app.tabs[0].id;
        let _ = app.update(Message::EditBlockLoaded {
            tab,
            id: "c2".into(),
            result: Ok(None),
        });
        let opens = |messages: &[Message]| {
            messages
                .iter()
                .any(|m| matches!(m, Message::CommitModalOpen))
        };
        let header = {
            let mut sim = simulator(&app);
            let found = sim.find("commit:").unwrap();
            iced_test::selector::Bounded::bounds(&found)
        };
        let top = header.y + header.height;
        // A click on the text (left part of the box) does not open it.
        let text_clicks = clicks_along(&app, header.x - 60.0, top + 30.0, 120.0);
        assert!(!opens(&text_clicks), "{text_clicks:?}");
        let button = |app: &App| {
            let mut messages = Vec::new();
            let mut y = header.y;
            while y < top {
                messages.extend(clicks_along(app, SIZE.0 - 90.0, y, 90.0));
                y += 4.0;
            }
            messages
        };
        assert!(opens(&button(&app)));
        let _ = app.update(Message::CommitModalOpen);
        assert_eq!(app.message_modal.as_ref().unwrap().read_only, None);
        // A click on the modal itself does not close it; one beside it does.
        let messages = clicks_along(&app, SIZE.0 / 2.0 - 300.0, SIZE.1 / 2.0, 20.0);
        assert!(
            !messages
                .iter()
                .any(|m| matches!(m, Message::CommitModalClose)),
            "{messages:?}"
        );
        let messages = clicks_along(&app, 20.0, 20.0, 10.0);
        assert!(messages
            .iter()
            .any(|m| matches!(m, Message::CommitModalClose)));
        app.message_modal = None;
        // Blocked: the open icon opens the modal read-only.
        let reason = "Check out a branch that contains this commit";
        let _ = app.update(Message::EditBlockLoaded {
            tab,
            id: "c2".into(),
            result: Ok(Some(reason.into())),
        });
        assert!(opens(&button(&app)));
        let _ = app.update(Message::CommitModalOpen);
        assert_eq!(
            app.message_modal.as_ref().unwrap().read_only.as_deref(),
            Some(reason)
        );
    }

    /// Finding #43: the hovered header with its button (pencil, open
    /// icon), the modal in edit mode with the note about other refs and
    /// read-only (pushed, stash); Dark and Light.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_message_modal() {
        use crate::settings::ThemeChoice;
        let hovered = |app: &App, name: &str| {
            let path = fresh_path(name);
            let mut sim = simulator(app);
            let found = sim.find("commit:").unwrap();
            let header = iced_test::selector::Bounded::bounds(&found);
            let at = iced::Point::new(header.x, header.y + header.height + 30.0);
            sim.point_at(at);
            let _ = sim.simulate(vec![iced::Event::Mouse(iced::mouse::Event::CursorMoved {
                position: at,
            })]);
            let snap = sim.snapshot(&app.theme()).unwrap();
            assert!(snap.matches_image(&path).unwrap());
        };
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            let mut app = details_app(&[
                (
                    "c2",
                    "Fix the typo in the header",
                    "Explain why.\n\nRefs #12",
                ),
                ("c1", "Initial", ""),
            ]);
            app.settings.appearance.theme = choice.clone();
            let tab = app.tabs[0].id;
            let _ = app.update(Message::CommitSelected("c2".into()));
            let _ = app.update(Message::EditBlockLoaded {
                tab,
                id: "c2".into(),
                result: Ok(None),
            });
            hovered(&app, &format!("message-button-edit-{theme}"));
            let _ = app.update(Message::CommitModalOpen);
            let _ = app.update(Message::CommitModalFactsLoaded {
                tab,
                id: "c2".into(),
                result: Ok(git_core::reword::EditFacts {
                    branch: "main".into(),
                    keep_old: vec!["backup".into(), "v1.2".into()],
                    drops_signature: true,
                }),
            });
            snapshot(&app, &format!("message-modal-edit-{theme}"));
            let _ = app.update(Message::CommitModalSummary("Fix the typo".into()));
            let _ = app.update(Message::CommitModalClose);
            snapshot(&app, &format!("message-modal-discard-{theme}"));
            app.message_modal = None;
            // Pushed: read-only with the reason.
            let _ = app.update(Message::EditBlockLoaded {
                tab,
                id: "c2".into(),
                result: Ok(Some(
                    git_core::reword::EditBlock::Pushed("origin/main".into()).reason(),
                )),
            });
            hovered(&app, &format!("message-button-open-{theme}"));
            let _ = app.update(Message::CommitModalOpen);
            snapshot(&app, &format!("message-modal-pushed-{theme}"));
            // A stash.
            let mut app = crate::app::tests::graph_app();
            app.settings.appearance.theme = choice.clone();
            let _ = app.update(Message::CommitSelected("s".into()));
            let _ = app.update(Message::CommitModalOpen);
            snapshot(&app, &format!("message-modal-stash-{theme}"));
        }
    }

    /// M15-11: the message modal next to the commit panel (same parts,
    /// paddings and button positions) and the "Change author" dialog,
    /// Dark and Light.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_change_author() {
        use crate::settings::ThemeChoice;
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            let mut app = details_app(&[
                ("c2", "Fix the typo in the header", "Explain why."),
                ("c1", "Initial", ""),
            ]);
            app.settings.appearance.theme = choice.clone();
            app.settings.identities.list = vec![crate::settings::SavedIdentity {
                name: "Bo Kim".into(),
                email: "bo@example.com".into(),
            }];
            let tab = app.tabs[0].id;
            let _ = app.update(Message::CommitSelected("c2".into()));
            let _ = app.update(Message::EditBlockLoaded {
                tab,
                id: "c2".into(),
                result: Ok(None),
            });
            let _ = app.update(Message::CommitModalOpen);
            snapshot(&app, &format!("change-author-editor-{theme}"));
            let _ = app.update(Message::EditAuthorChosen(Some(git_core::Identity {
                name: "Bo Kim".into(),
                email: "bo@example.com".into(),
            })));
            snapshot(&app, &format!("change-author-editor-chosen-{theme}"));
            // The commit panel for comparison.
            app.message_modal = None;
            let _ = app.update(Message::WipSelected);
            snapshot(&app, &format!("change-author-commit-panel-{theme}"));
            // The dialog.
            let _ = app.update(Message::ChangeAuthorStart(vec!["c2".into(), "c1".into()]));
            if app.form.is_none() {
                // The fixture is detached: open the dialog directly.
                let view = app.tabs[0].view_mut().unwrap();
                let mut snapshot = (*view.snapshot).clone();
                snapshot.head = git_core::Head::Branch {
                    name: "main".into(),
                    target: "c2".into(),
                };
                view.snapshot = std::sync::Arc::new(snapshot);
                let _ = app.update(Message::ChangeAuthorStart(vec!["c2".into(), "c1".into()]));
            }
            let _ = app.update(Message::ChangeAuthorFactsLoaded {
                tab,
                ids: vec!["c2".into(), "c1".into()],
                result: Ok(git_core::reword::EditFacts {
                    branch: "main".into(),
                    keep_old: vec!["v1.0".into()],
                    drops_signature: false,
                }),
            });
            snapshot(&app, &format!("change-author-dialog-{theme}"));
        }
    }

    /// `ready_app` whose commits are `ages` seconds old (newest first).
    fn aged_app(ages: &[i64], now: i64) -> App {
        let ids: Vec<String> = (0..ages.len()).map(|i| format!("c{i}")).collect();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let mut app = ready_app(&refs);
        app.now = now;
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        let mut commits = (*snapshot.commits).clone();
        for (i, (commit, age)) in commits.iter_mut().zip(ages).enumerate() {
            commit.author_time = now - age;
            commit.summary = format!("Commit {i}");
            commit.author = "Ana Lyst".into();
        }
        snapshot.commits = std::sync::Arc::new(commits);
        view.snapshot = std::sync::Arc::new(snapshot);
        app
    }

    /// Markers only where the relative date changes (M14-06).
    #[test]
    fn time_markers_where_the_label_changes() {
        let now = 1_794_484_800; // 2026-11-12 12:00 UTC
        let app = aged_app(
            &[10, 30, 600, 7_200, 7_300, 90_000, 400_000, 4_000_000],
            now,
        );
        let snapshot = &app.tabs[0].view().unwrap().snapshot;
        let markers: Vec<Option<String>> = (0..8)
            .map(|i| crate::ui::graph::time_marker(snapshot, i, now))
            .collect();
        assert_eq!(
            markers,
            [
                Some("just now".into()),
                None,
                Some("10 minutes ago".into()),
                Some("2 hours ago".into()),
                None,
                Some("yesterday".into()),
                Some("5 days ago".into()),
                Some("1 month ago".into()),
            ]
        );
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_time_markers() {
        let now = 1_794_484_800;
        let day = 86_400;
        let mut app = aged_app(
            &[
                5,
                40,
                1_500,
                1_600,
                10_000,
                day + 3_600,
                3 * day,
                3 * day + 10,
                9 * day,
                70 * day,
                400 * day,
                800 * day,
            ],
            now,
        );
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            snapshot(&app, &format!("time-markers-{theme}"));
        }
    }

    /// Time markers of 100 000 rows (only visible rows are built per
    /// frame): `cargo test -p gitloom --release perf_time_markers --
    /// --ignored --nocapture`.
    #[test]
    #[ignore = "performance measurement"]
    fn perf_time_markers() {
        let now = 1_794_484_800;
        let ages: Vec<i64> = (0..100_000).map(|i| i * 600).collect();
        let app = aged_app(&ages, now);
        let snapshot = &app.tabs[0].view().unwrap().snapshot;
        let start = std::time::Instant::now();
        let shown = (0..100_000)
            .filter(|&i| crate::ui::graph::time_marker(snapshot, i, now).is_some())
            .count();
        let all = start.elapsed();
        let start = std::time::Instant::now();
        for i in 0..60 {
            let _ = crate::ui::graph::time_marker(snapshot, i, now);
        }
        let visible = start.elapsed();
        eprintln!(
            "time markers: all 100k rows {all:?} ({shown} markers), 60 visible rows {visible:?}"
        );
    }

    /// A click on "commit: <short sha>" copies the full sha (M14-01).
    /// Layout only, so part of every test run.
    #[test]
    fn details_header_click_copies_the_full_sha() {
        let full = "0123456789abcdef0123456789abcdef01234567";
        let mut app = details_app(&[(full, "Summary", ""), ("c1", "Initial", "")]);
        let _ = app.update(Message::CommitSelected(full.into()));
        let messages: Vec<Message> = {
            let mut sim = simulator(&app);
            sim.click("commit:")
                .unwrap_or_else(|e| panic!("no header: {e}"));
            sim.into_messages().collect()
        };
        assert!(
            messages
                .iter()
                .any(|m| matches!(m, Message::CopyToClipboard(t) if t == full)),
            "{messages:?}"
        );
        for message in messages {
            let _ = app.update(message);
        }
        assert_eq!(app.status, "Copied to the clipboard");
    }

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_details_message_box() {
        let long_body = (1..=40)
            .map(|i| format!("Line {i} of a long description that wraps in the panel."))
            .collect::<Vec<_>>()
            .join("\n");
        let mut app = details_app(&[
            ("c3", "Fix the graph header", ""),
            ("c2", "Add the message box", &long_body),
            ("c1", "Initial commit", ""),
        ]);
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            for (id, name) in [("c3", "one-line"), ("c2", "long")] {
                let _ = app.update(Message::CommitSelected(id.into()));
                snapshot(&app, &format!("details-message-{name}-{theme}"));
            }
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
            source: git_core::prompt::PromptSource::Staged,
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

    /// Preferences → External tools with the command field (fix after
    /// M15): a preset for diffs, a custom merge command with a full path,
    /// the tool list Git offers, and the empty field "Define tool…" opens.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_external_tools() {
        use crate::settings::{ToolPick, ToolPreset};
        let mut app = ready_app(&["c"]);
        let _ = app.update(Message::SettingsOpenedAt(
            crate::settings::Category::ExternalTools,
        ));
        let info = git_core::ToolInfo {
            configured: None,
            available: vec!["meld".into(), "kdiff3".into(), "vimdiff".into()],
        };
        let _ = app.update(Message::ToolsLoaded(Ok((info.clone(), info))));
        let _ = app.update(Message::DiffToolChosen(ToolPick::Preset(
            ToolPreset::VsCode,
        )));
        let _ = app.update(Message::MergeToolChosen(ToolPick::Custom));
        let _ = app.update(Message::MergeToolCommandInput(
            r#""C:\Program Files\WinMerge\WinMergeU.exe" -u -e {local} {remote} {merged}"#.into(),
        ));
        snapshot(&app, "preferences-external-tools");
        // "Define tool…" for diffs with no tool: the empty field, focused.
        let _ = app.update(Message::DiffToolChosen(ToolPick::None));
        let _ = app.update(Message::DefineTool { merge: false });
        snapshot_focused(
            &app,
            crate::ui::settings::tool_input(git_core::ToolKind::Diff),
            "preferences-external-tools-define",
        );
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
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
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
        use crate::settings::ThemeChoice;
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            let mut app = ready_app(&["c"]);
            app.settings.appearance.theme = choice.clone();
            app.settings.tools.editor_command = "code {path}".into();
            let view = app.tabs[0].view_mut().unwrap();
            let mut repo = (*view.snapshot).clone();
            repo.repo_editor = Some("idea {path}".into());
            view.apply_snapshot(std::sync::Arc::new(repo));
            let _ = app.update(Message::SettingsOpenedAt(
                crate::settings::Category::RepoEditor,
            ));
            snapshot(&app, &format!("repository-editor-{theme}"));
            // M15-02: a long global command; back to the global setting.
            app.settings.tools.editor_command =
                r#""C:\Users\maintainer\AppData\Local\Programs\Zed\Zed.exe" --new-window {path}"#
                    .into();
            let label = crate::app::use_global_editor_label(&app.settings.tools.editor_command);
            let _ = app.update(Message::RepoEditorChosen("Custom".into()));
            let _ = app.update(Message::RepoEditorChosen(label));
            let view = app.tabs[0].view_mut().unwrap();
            let mut repo = (*view.snapshot).clone();
            repo.repo_editor = None;
            view.apply_snapshot(std::sync::Arc::new(repo));
            snapshot(&app, &format!("repository-editor-global-{theme}"));
        }
    }

    /// Stash node and label (M11-05): dashed square with the archive icon,
    /// "WIP on main" in the graph, STASHES and the details title; Dark and
    /// Light.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_stash_node() {
        use crate::settings::ThemeChoice;
        let mut app = crate::app::tests::graph_app();
        let _ = app.update(Message::CommitSelected("s".into()));
        for (choice, name) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            app.settings.appearance.theme = choice.clone();
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

    /// Word wrap (M11-07) in Hunk, Inline and Split; the wrapped line keeps
    /// its number and marker on the first row.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_word_wrap() {
        use crate::settings::DiffMode;
        for (mode, name) in [
            (DiffMode::Hunk, "hunk"),
            (DiffMode::Inline, "inline"),
            (DiffMode::Split, "split"),
        ] {
            let mut app = crate::app::tests::wide_diff_app(mode);
            let _ = app.update(Message::EditorWordWrapToggled(true));
            snapshot(&app, &format!("word-wrap-{name}"));
        }
    }

    /// Syntax highlighting (M11-09): a Rust diff in every mode and theme,
    /// and the conflict view.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_highlighting() {
        use crate::settings::{DiffMode, ThemeChoice};
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            for (mode, name) in [
                (DiffMode::Hunk, "hunk"),
                (DiffMode::Inline, "inline"),
                (DiffMode::Split, "split"),
            ] {
                let app = crate::app::tests::rust_diff_app(mode, choice.clone());
                snapshot(&app, &format!("highlight-{name}-{theme}"));
            }
        }
        let app = crate::app::tests::rust_conflict_app();
        snapshot(&app, "highlight-conflict-dark");
    }

    /// Word diff (fix after M15): changed characters of paired lines in
    /// Hunk, Inline and Split, Dark and Light, with syntax colors kept.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_word_diff() {
        use crate::settings::{DiffMode, ThemeChoice};
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            for (mode, name) in [
                (DiffMode::Hunk, "hunk"),
                (DiffMode::Inline, "inline"),
                (DiffMode::Split, "split"),
            ] {
                let mut app = crate::app::tests::word_diff_app(mode, choice.clone());
                snapshot(&app, &format!("word-diff-{name}-{theme}"));
                if mode == DiffMode::Inline {
                    let _ = app.update(Message::EditorWordWrapToggled(true));
                    snapshot(&app, &format!("word-diff-{name}-wrapped-{theme}"));
                }
            }
        }
    }

    /// Start screen like GitKraken's New Tab, both themes at 100 and 125 %.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_start_screen() {
        use crate::settings::ThemeChoice;
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            let mut app = start_app();
            app.settings.appearance.theme = choice.clone();
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

    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_staged_file_menu() {
        let mut app = crate::app::tests::staging_app(&["a.txt"], &["src/main.rs"]);
        let _ = app.update(Message::FileContextMenu {
            file: git_core::FileChange {
                path: "src/main.rs".into(),
                kind: git_core::ChangeKind::Modified,
                old_path: None,
            },
            area: crate::app::FileArea::Staged,
            position: iced::Point::new(1100.0, 200.0),
        });
        snapshot(&app, "file-menu-staged");
    }

    /// Messages of a click at every few pixels of the row showing `label`
    /// while the pointer hovers it (the hover buttons appear then).
    fn row_clicks(app: &App, label: &str) -> Vec<Message> {
        row_clicks_over(app, label, 360.0)
    }

    /// [`row_clicks`] over `width` pixels from the label.
    fn row_clicks_over(app: &App, label: &str, width: f32) -> Vec<Message> {
        use iced::mouse::{Button, Event as Mouse};
        use iced::{Event, Point};
        let bounds = {
            let mut sim = simulator(app);
            let found = sim
                .find(label)
                .unwrap_or_else(|e| panic!("no row {label:?}: {e}"));
            iced_test::selector::Bounded::bounds(&found)
        };
        let mut messages = Vec::new();
        let mut x = bounds.x;
        while x < bounds.x + width {
            let at = Point::new(x, bounds.center_y());
            let mut sim = simulator(app);
            sim.point_at(at);
            let _ = sim.simulate(vec![
                Event::Mouse(Mouse::CursorMoved { position: at }),
                Event::Mouse(Mouse::ButtonPressed(Button::Left)),
                Event::Mouse(Mouse::ButtonReleased(Button::Left)),
            ]);
            messages.extend(sim.into_messages());
            x += 4.0;
        }
        messages
    }

    /// Finding #44: the staging area at the bottom keeps every function:
    /// Stage all, Unstage all, Discard all (with its confirmation), the
    /// row hover buttons, context menus, Amend, the commit button; a click
    /// on a file opens its diff in the center above the panel. Layout
    /// only, part of every run.
    #[test]
    fn bottom_staging_keeps_its_functions() {
        use crate::panes::DetailsPanelPosition;
        let mut app = crate::app::tests::staging_app(&["a.txt", "b.txt"], &["src/main.rs"]);
        app.panes.set_position(DetailsPanelPosition::Bottom);
        // Enough room for every part at the test window's height.
        app.panes.set_ratios(crate::panes::PaneRatios {
            bottom: 0.3,
            ..app.panes.ratios
        });
        let _ = app.update(Message::WipSelected);
        let _ = app.update(Message::SummaryChanged("Summary".into()));
        let has = |messages: &[Message], test: &dyn Fn(&Message) -> bool| messages.iter().any(test);
        let m = clicked(&app, "Stage all changes");
        assert!(has(&m, &|m| matches!(m, Message::StageAll)), "{m:?}");
        let m = clicked(&app, "Unstage all changes");
        assert!(has(&m, &|m| matches!(m, Message::UnstageAll)), "{m:?}");
        let m = clicked(&app, "Amend previous commit");
        assert!(
            has(&m, &|m| matches!(m, Message::AmendToggled(true))),
            "{m:?}"
        );
        let m = clicked(&app, "Commit changes to 1 file");
        assert!(has(&m, &|m| matches!(m, Message::CommitRequested)), "{m:?}");
        let m = row_clicks_over(&app, "a.txt", 600.0);
        assert!(
            has(&m, &|m| matches!(m, Message::Stage(f) if f.path == "a.txt")),
            "{m:?}"
        );
        let m = row_clicks_over(&app, "src/main.rs", 600.0);
        assert!(
            has(
                &m,
                &|m| matches!(m, Message::Unstage(f) if f.path == "src/main.rs")
            ),
            "{m:?}"
        );
        // Discard all: the trash icon at the left of the header.
        let title = {
            let mut sim = simulator(&app);
            let found = sim.find("3 file changes on HEAD (c)").unwrap();
            iced_test::selector::Bounded::bounds(&found)
        };
        let m = clicks_along(&app, 0.0, title.center_y(), SIZE.0 / 2.0);
        assert!(
            has(&m, &|m| matches!(
                m,
                Message::Discard(crate::app::DiscardTarget::All)
            )),
            "{m:?}"
        );
        // Right click on a row and on the staged header open their menus.
        let right_click = |label: &str| {
            let mut sim = simulator(&app);
            let found = sim.find(label).unwrap();
            let at = iced_test::selector::Bounded::bounds(&found).center();
            sim.point_at(at);
            let _ = sim.simulate(vec![
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position: at }),
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Right,
                )),
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Right,
                )),
            ]);
            sim.into_messages().collect::<Vec<_>>()
        };
        let m = right_click("b.txt");
        assert!(
            has(&m, &|m| matches!(m, Message::FileContextMenu { .. })),
            "{m:?}"
        );
        let m = right_click("Staged Files (1)");
        assert!(
            has(&m, &|m| matches!(m, Message::StagedHeaderMenu(_))),
            "{m:?}"
        );
        // The diff opens in the center; the panel stays below.
        let m = clicked(&app, "b.txt");
        let open = m
            .into_iter()
            .find(|m| matches!(m, Message::DiffOpened(_)))
            .expect("diff");
        let _ = app.update(open);
        let view = app.tabs[0].view().unwrap();
        assert!(matches!(view.center, crate::repo_view::CenterView::Diff(_)));
        assert!(simulator(&app).find("Unstaged Files (2)").is_ok());
    }

    /// Bounds of the widget showing `label` in a window of `size`.
    fn bounds_in(app: &App, size: (f32, f32), label: &str) -> iced::Rectangle {
        let mut sim = Simulator::with_size(settings(), size, app.view());
        let found = sim
            .find(label)
            .unwrap_or_else(|e| panic!("no {label:?}: {e}"));
        iced_test::selector::Bounded::bounds(&found)
    }

    /// Finding #45: `layout::CHROME_HEIGHT` is the measured height of the
    /// tab bar, toolbar, status bar and their gaps. Layout only.
    #[test]
    fn chrome_height_is_measured() {
        let mut app = crate::app::tests::staging_app(&["a.txt"], &["b.txt"]);
        let _ = app.update(Message::WipSelected);
        // Right: the staging header text sits 10 px below the pane top.
        let title = bounds_in(&app, SIZE, "2 file changes on HEAD (c)");
        let version = bounds_in(&app, SIZE, concat!("v", env!("CARGO_PKG_VERSION")));
        let pane_top = title.y - 10.0;
        // Status bar: 26 px (padding 3, the 20 px toggles), the text in
        // its middle; 1 px gap above the bar.
        let pane_bottom = version.center_y() - 13.0 - 1.0;
        let chrome = SIZE.1 - (pane_bottom - pane_top);
        assert!(
            (chrome - crate::layout::CHROME_HEIGHT).abs() <= 1.5,
            "measured {chrome}"
        );
    }

    /// Finding #45: at the window minimum (100 % zoom) nothing of the
    /// staging area is cut off in either position: both lists show their
    /// header and two rows, the commit message its fields, the Commit
    /// button and a hint stay inside the panel. Layout only.
    #[test]
    fn staging_fits_at_the_window_minimum() {
        use crate::panes::DetailsPanelPosition;
        let min = crate::layout::window_min();
        let size = (min.width, min.height);
        for position in [DetailsPanelPosition::Right, DetailsPanelPosition::Bottom] {
            let mut app = crate::app::tests::staging_app(
                &["a1.txt", "a2.txt", "a3.txt", "a4.txt"],
                &["s1.txt", "s2.txt", "s3.txt"],
            );
            app.panes.set_position(position);
            let _ = app.update(Message::WindowResized(min));
            let _ = app.update(Message::WipSelected);
            let status = bounds_in(&app, size, concat!("v", env!("CARGO_PKG_VERSION")));
            let commit = bounds_in(&app, size, "Commit changes to 3 files");
            assert!(
                commit.y + commit.height <= status.y - 3.0,
                "{position:?}: commit button {commit:?} below the panel"
            );
            let hint = bounds_in(&app, size, "Enter a commit summary");
            assert!(
                hint.y + hint.height <= status.y,
                "{position:?}: hint {hint:?}"
            );
            let summary_label = match position {
                DetailsPanelPosition::Right => "Commit",
                DetailsPanelPosition::Bottom => "Commit Message",
            };
            let label = bounds_in(&app, size, summary_label);
            let mut sim = Simulator::with_size(settings(), size, app.view());
            let lists = scrolled(&mut sim);
            drop(sim);
            for (header, second) in [
                ("Unstaged Files (4)", "a2.txt"),
                ("Staged Files (3)", "s2.txt"),
            ] {
                let header = bounds_in(&app, size, header);
                let row = bounds_in(&app, size, second);
                assert!(row.y > header.y, "{position:?}");
                // Finding #47: the frame shows both rows completely, with
                // the frame's padding below them.
                let list = lists
                    .iter()
                    .find(|l| l.bounds.contains(row.center()))
                    .unwrap_or_else(|| {
                        panic!("{position:?}: no list around {second} {row:?} in {lists:?}")
                    });
                assert!(
                    list.bounds.height
                        >= 2.0 * crate::ui::panel::FILE_ROW_HEIGHT
                            + crate::ui::panel::FILE_FRAME_CHROME
                            - 2.0
                            - 0.5,
                    "{position:?}: {second}'s list is {list:?}"
                );
                assert!(
                    row.y + row.height <= list.bounds.y + list.bounds.height,
                    "{position:?}: {second} {row:?} cut off in {list:?}"
                );
                let limit = match position {
                    // At the bottom the lists end above the message.
                    DetailsPanelPosition::Bottom => label.y,
                    // At the right the staged list ends above the box.
                    DetailsPanelPosition::Right => label.y,
                };
                assert!(
                    row.y + row.height <= limit,
                    "{position:?}: {second} {row:?} cut off above {limit}"
                );
            }
            // The graph keeps its minimum height above the panel.
            if position == DetailsPanelPosition::Bottom {
                let graph = bounds_in(&app, size, "BRANCH / TAG");
                let header = bounds_in(&app, size, "Unstaged Files (4)");
                assert!(
                    header.y - graph.y >= crate::layout::BOTTOM_GRAPH_MIN_HEIGHT,
                    "graph {graph:?}, panel {header:?}"
                );
            }
        }
    }

    /// The apps of the minimum size checks: (name, app) for the graph with
    /// details right and at the bottom, staging right and at the bottom,
    /// a diff, the conflict view and the large modals.
    fn minimum_size_apps() -> Vec<(String, App, Vec<&'static str>)> {
        use crate::panes::DetailsPanelPosition;
        let min = crate::layout::window_min();
        let sized = |mut app: App| {
            let _ = app.update(Message::WindowResized(min));
            app
        };
        let mut out = Vec::new();
        for (position, name) in [
            (DetailsPanelPosition::Right, "right"),
            (DetailsPanelPosition::Bottom, "bottom"),
        ] {
            let mut app = file_list_app();
            app.panes.set_position(position);
            out.push((format!("details-{name}"), sized(app), vec!["commit:"]));
            let mut app = crate::app::tests::staging_app(
                &["src/main.rs", "src/ui/panel.rs", "README.md"],
                &["Cargo.toml", "src/lib.rs"],
            );
            app.panes.set_position(position);
            let _ = app.update(Message::WipSelected);
            out.push((
                format!("staging-{name}"),
                sized(app),
                vec!["Commit changes to 2 files", "Stage all changes"],
            ));
        }
        let app = crate::app::tests::minimap_app(crate::settings::DiffMode::Hunk);
        out.push(("diff".into(), sized(app), vec![]));
        let app = crate::app::tests::rust_conflict_app();
        out.push(("conflict".into(), sized(app), vec![]));
        // Modals.
        let mut app = crate::app::tests::ready_app(&["c"]);
        let _ = app.update(Message::SettingsOpened);
        out.push(("preferences".into(), sized(app), vec![]));
        let mut app = start_app();
        let _ = app.update(Message::CloneStart);
        out.push(("clone".into(), sized(app), vec!["Clone", "Cancel"]));
        let mut app = details_app(&[
            ("c2", "Fix the typo", "Why.\n\nMore."),
            ("c1", "Initial", ""),
        ]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::CommitSelected("c2".into()));
        let _ = app.update(Message::EditBlockLoaded {
            tab,
            id: "c2".into(),
            result: Ok(None),
        });
        let _ = app.update(Message::ChangeAuthorStart(vec!["c2".into()]));
        if app.form.is_none() {
            // The fixture is detached: on a branch the dialog opens.
            let view = app.tabs[0].view_mut().unwrap();
            let mut snapshot = (*view.snapshot).clone();
            snapshot.head = git_core::Head::Branch {
                name: "main".into(),
                target: "c2".into(),
            };
            view.snapshot = std::sync::Arc::new(snapshot);
            let _ = app.update(Message::ChangeAuthorStart(vec!["c2".into()]));
        }
        out.push((
            "change-author".into(),
            sized(app),
            vec!["Change author", "Cancel"],
        ));
        let mut app = details_app(&[
            (
                "c2",
                "Fix the typo",
                &"A long line of the description.\n".repeat(30),
            ),
            ("c1", "Initial", ""),
        ]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::CommitSelected("c2".into()));
        let _ = app.update(Message::EditBlockLoaded {
            tab,
            id: "c2".into(),
            result: Ok(None),
        });
        let _ = app.update(Message::CommitModalOpen);
        let _ = app.update(Message::CommitModalFactsLoaded {
            tab,
            id: "c2".into(),
            result: Ok(git_core::reword::EditFacts {
                branch: "main".into(),
                keep_old: vec!["backup".into(), "v1.2".into()],
                drops_signature: true,
            }),
        });
        out.push(("message-edit".into(), sized(app), vec!["Save", "Cancel"]));
        let mut app = details_app(&[
            (
                "c2",
                "Fix the typo",
                &"A long line of the description.\n".repeat(30),
            ),
            ("c1", "Initial", ""),
        ]);
        let tab = app.tabs[0].id;
        let _ = app.update(Message::CommitSelected("c2".into()));
        let _ = app.update(Message::EditBlockLoaded {
            tab,
            id: "c2".into(),
            result: Ok(Some(
                git_core::reword::EditBlock::Pushed("origin/main".into()).reason(),
            )),
        });
        let _ = app.update(Message::CommitModalOpen);
        out.push((
            "message-read-only".into(),
            sized(app),
            vec!["Copy message", "Close"],
        ));
        let mut app = crate::app::tests::ready_app(&["c"]);
        let input = git_core::prompt::PromptInput {
            source: git_core::prompt::PromptSource::Staged,
            branch: "main".into(),
            recent: vec!["Add login form".into()],
            files: vec![git_core::prompt::PromptFile::new(
                "M",
                "src/login.rs",
                Some((3, 1)),
                Ok("diff --git a/src/login.rs b/src/login.rs\n@@ -1 +1 @@\n-old\n+new\n".into()),
            )],
        };
        let prompt = git_core::prompt::build(git_core::prompt::DEFAULT_INSTRUCTION, None, &input);
        let _ = app.update(Message::PromptReady(Ok(prompt)));
        out.push(("commit-prompt".into(), sized(app), vec!["Copy", "Close"]));
        out
    }

    /// Finding #45: at the window minimum (100 % zoom) the buttons of the
    /// staging area and of the large modals lie inside the window. Layout
    /// only, part of every run.
    #[test]
    fn buttons_fit_at_the_window_minimum() {
        let min = crate::layout::window_min();
        for (name, app, labels) in minimum_size_apps() {
            for label in labels {
                let found = bounds_in(&app, (min.width, min.height), label);
                assert!(
                    found.x >= 0.0
                        && found.y >= 0.0
                        && found.x + found.width <= min.width
                        && found.y + found.height <= min.height - 26.0,
                    "{name}: {label:?} at {found:?} outside {min:?}"
                );
            }
        }
    }

    /// Finding #45: snapshots at the window minimum, 100 % zoom, Dark.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_minimum_size() {
        let min = crate::layout::window_min();
        for (name, app, _) in minimum_size_apps() {
            snapshot_sized(&app, &format!("minimum-{name}"), (min.width, min.height));
        }
    }

    fn finish_requests(messages: &[Message]) -> Vec<String> {
        let mut names: Vec<String> = messages
            .iter()
            .filter_map(|m| match m {
                Message::FlowFinishRequested(name) => Some(name.clone()),
                _ => None,
            })
            .collect();
        names.dedup();
        names
    }

    /// GITFLOW rows (M15-04): the hover flag of feature, release and hotfix
    /// rows opens the finish dialog; development and production have none;
    /// during a merge the flag is disabled. Layout only, so part of every
    /// test run.
    #[test]
    fn gitflow_rows_finish_by_the_hover_flag() {
        let mut app = crate::app::tests::flow_app();
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        snapshot.refs.push(git_core::Reference {
            name: "hotfix/1.0.1".into(),
            kind: git_core::RefKind::LocalBranch,
            target: "c".into(),
            is_head: false,
            upstream: None,
            ahead_behind: None,
        });
        view.snapshot = std::sync::Arc::new(snapshot);
        for (label, branch, title) in [
            ("login", "feature/login", "Finish feature login"),
            ("1.2.0", "release/1.2.0", "Finish release 1.2.0"),
            ("1.0.1", "hotfix/1.0.1", "Finish hotfix 1.0.1"),
        ] {
            assert_eq!(finish_requests(&row_clicks(&app, label)), [branch]);
            let _ = app.update(Message::FlowFinishRequested(branch.into()));
            assert_eq!(app.form.as_ref().map(|f| f.title.as_str()), Some(title));
            app.form = None;
        }
        for label in ["develop", "main"] {
            assert!(
                finish_requests(&row_clicks(&app, label)).is_empty(),
                "{label}"
            );
        }
        // During a merge: disabled, with the reason of the menu entry.
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = (*view.snapshot).clone();
        snapshot.state = git_core::RepoState::Merging {
            merge_head: "x".into(),
        };
        view.snapshot = std::sync::Arc::new(snapshot);
        assert!(finish_requests(&row_clicks(&app, "login")).is_empty());
        let reason = "Cannot merge while a merge is in progress";
        assert_eq!(
            app.tabs[0].view().unwrap().finish_block().as_deref(),
            Some(reason)
        );
        let _ = app.update(Message::BranchContextMenu {
            name: "feature/login".into(),
            kind: git_core::RefKind::LocalBranch,
            position: iced::Point::ORIGIN,
        });
        let menu = app.context_menu.take().unwrap();
        assert!(menu.entries.iter().any(|e| matches!(e,
            crate::overlay::MenuEntry::Item { label, enabled: false, hint: Some(h), .. }
                if label == "Finish feature login" && h == reason)));
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

    /// A table wider than its viewport (M11-16): header and rows move by the
    /// same offset and stay aligned; the vertical scrollbar stays at the
    /// right edge of the viewport. Layout only.
    #[test]
    fn graph_table_scrolls_header_and_rows_together() {
        let mut app = crate::app::tests::graph_app();
        app.settings.appearance.graph_columns.message = 1500.0;
        let positions = |app: &App| {
            let id = app.tabs[0].view().unwrap().graph_scroll_id.clone();
            let mut sim = simulator(app);
            let mut x = |label: &str| {
                let target = sim
                    .find(label)
                    .unwrap_or_else(|e| panic!("no {label}: {e}"));
                iced_test::selector::Bounded::bounds(&target).x
            };
            let (header, row) = (x("AUTHOR"), x("Bo Dev"));
            let list = sim.find(id).unwrap_or_else(|e| panic!("no list: {e}"));
            let list = iced_test::selector::Bounded::bounds(&list);
            (header, row, list.x + list.width)
        };
        let (header, row, right) = positions(&app);
        assert!((header - row).abs() < 0.5, "{header} vs {row}");
        let _ = app.update(Message::GraphHScrolled {
            offset: 200.0,
            max: 2000.0,
        });
        let (header2, row2, right2) = positions(&app);
        eprintln!(
            "DEBUG {header} {row} {right} -> {header2} {row2} {right2} off={}",
            app.tabs[0].view().unwrap().graph_h_offset
        );
        assert!(
            (header - 200.0 - header2).abs() < 0.5,
            "{header} → {header2}"
        );
        assert!((row - 200.0 - row2).abs() < 0.5, "{row} → {row2}");
        assert_eq!(right, right2, "the vertical scrollbar stays in place");
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

    /// Performance of highlighting and word wrap (M11-10): highlighting a
    /// 1 MB Rust file, the plain fallback of a 2 MB file, and frames of a
    /// wrapped 2 MB diff at several scroll positions (layout of the whole
    /// window). Run in release:
    /// `cargo test -p gitloom --release perf_highlight_and_wrap -- --ignored --nocapture`.
    #[test]
    #[ignore = "performance measurement"]
    fn perf_highlight_and_wrap() {
        use std::time::Instant;
        let line = |i: usize| {
            format!(
                "    let value_{i} = compute(\"text {i}\", {i}); // comment {i} with some words\n"
            )
        };
        let mut one_mb = String::from("fn main() {\n");
        while one_mb.len() < 1000 * 1024 - 200 {
            one_mb.push_str(&line(one_mb.len()));
        }
        one_mb.push_str("}\n");
        let lines = one_mb.lines().count();
        let start = Instant::now();
        let outcome =
            crate::highlight::highlight("big.rs", &one_mb, crate::highlight::CodeTheme::DARK);
        eprintln!(
            "PERF highlight 1 MB ({lines} lines): {:?} ({})",
            start.elapsed(),
            matches!(outcome, crate::highlight::Outcome::Highlighted(_))
        );
        let too_large = one_mb.repeat(9);
        let start = Instant::now();
        let outcome =
            crate::highlight::highlight("big.rs", &too_large, crate::highlight::CodeTheme::DARK);
        eprintln!(
            "PERF highlight 9 MB (plain): {:?} ({outcome:?})",
            start.elapsed()
        );
        let two_mb = one_mb.repeat(2);

        // A wrapped 2 MB diff: every line changed.
        let mut app = crate::app::tests::staging_app(&["big.rs"], &[]);
        let _ = app.update(Message::DiffModeChanged(crate::settings::DiffMode::Inline));
        let _ = app.update(Message::EditorWordWrapToggled(true));
        let request = git_core::DiffRequest {
            kind: git_core::DiffKind::Unstaged,
            path: "big.rs".into(),
            old_path: None,
        };
        let _ = app.update(Message::DiffOpened(request.clone()));
        let new = two_mb.replace("comment", "remark");
        let start = Instant::now();
        let text = git_core::text_diff_with(
            two_mb.as_bytes(),
            new.as_bytes(),
            git_core::DiffContext::Full,
        );
        eprintln!("PERF diff of 2 MB: {:?}", start.elapsed());
        let tab = app.tabs[0].id;
        let start = Instant::now();
        let _ = app.update(Message::DiffLoaded {
            tab,
            request,
            context: git_core::DiffContext::Full,
            result: Ok(git_core::FileDiff {
                old_path: Some("big.rs".into()),
                new_path: Some("big.rs".into()),
                binary: false,
                too_large: false,
                encoding: None,
                hunks: text.hunks,
            }),
        });
        eprintln!(
            "PERF load wrapped diff ({} rows): {:?}",
            crate::app::tests::open_diff(&app).rows.len(),
            start.elapsed()
        );
        // Word diff (fix after M15): every line is a changed pair; the
        // background task clones the diff and pairs its lines.
        let start = Instant::now();
        let crate::repo_view::Loadable::Ready(file) = &crate::app::tests::open_diff(&app).diff
        else {
            panic!("diff not loaded");
        };
        let file = file.clone();
        let cloned = start.elapsed();
        let emphasis = crate::word_diff::of_diff(&file);
        eprintln!(
            "PERF word diff of the 2 MB diff (every line changed): {:?} (clone {cloned:?}, {} paired lines)",
            start.elapsed(),
            file.hunks.iter().flat_map(|h| &h.lines).filter(|l| !emphasis.ranges(l).is_empty()).count()
        );
        let _ = app.update(crate::app::tests::word_diff_result(&app));
        let rows = crate::app::tests::open_diff(&app).rows.len() as f32;
        let mut worst = std::time::Duration::ZERO;
        for step in 0..20 {
            let offset = step as f32 / 19.0 * rows * 20.0;
            let _ = app.update(Message::DiffScrolled {
                offset,
                height: 800.0,
                width: 1000.0,
            });
            let start = Instant::now();
            // View, layout and one update of the whole window.
            let mut sim = simulator(&app);
            let _ = sim.find("big.rs");
            worst = worst.max(start.elapsed());
        }
        eprintln!("PERF wrapped frame (view + layout), worst of 20 positions: {worst:?}");
    }

    /// Highlighting without a line limit (fix after M15): the first block
    /// and the whole of `crates/app/src/app.rs` and of a 100,000-line Rust
    /// file, and frames of the Inline diff of `app.rs` while the partial
    /// results arrive. Run in release:
    /// `cargo test -p gitloom --release perf_highlight_progressive -- --ignored --nocapture`.
    #[test]
    #[ignore = "performance measurement"]
    fn perf_highlight_progressive() {
        use std::sync::Arc;
        use std::time::Instant;
        let theme = crate::highlight::CodeTheme::DARK;
        let app_rs =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/app.rs")).unwrap();
        let synthetic: String = (0..100_000)
            .map(|i| match i % 4 {
                0 => format!("/// Doc {i}.\n"),
                1 => format!("fn f{i}(x: u32) -> u32 {{\n"),
                2 => format!("    x + {i} // \"{i}\"\n"),
                _ => "}\n".to_owned(),
            })
            .collect();
        for (name, text) in [("app.rs", &app_rs), ("100k lines", &synthetic)] {
            let lines = text.lines().count();
            let cache = std::sync::Mutex::new(crate::highlight::Cache::default());
            let job = crate::highlight::Job::new(theme, None);
            let start = Instant::now();
            let mut first = None;
            let mut rounds = 0;
            let files = vec![crate::highlight::Source::new("a.rs", text.clone(), None)];
            crate::highlight::run(&cache, files, theme, &job.watch(), |_| {
                first.get_or_insert(start.elapsed());
                rounds += 1;
            });
            eprintln!(
                "PERF highlight {name} ({lines} lines, {} KB): first block {:?}, complete {:?} ({rounds} blocks)",
                text.len() / 1024,
                first.unwrap(),
                start.elapsed()
            );
        }

        // Frames of the Inline diff of app.rs (every 10th line changed)
        // while the partial results arrive, at several scroll positions.
        let mut app = crate::app::tests::staging_app(&["app.rs"], &[]);
        let _ = app.update(Message::DiffModeChanged(crate::settings::DiffMode::Inline));
        let request = git_core::DiffRequest {
            kind: git_core::DiffKind::Unstaged,
            path: "app.rs".into(),
            old_path: None,
        };
        let _ = app.update(Message::DiffOpened(request.clone()));
        let new: String = app_rs
            .lines()
            .enumerate()
            .map(|(i, l)| {
                if i % 10 == 0 {
                    format!("{l} // changed\n")
                } else {
                    format!("{l}\n")
                }
            })
            .collect();
        let text = git_core::text_diff_with(
            app_rs.as_bytes(),
            new.as_bytes(),
            git_core::DiffContext::Full,
        );
        let tab = app.tabs[0].id;
        let _ = app.update(Message::DiffLoaded {
            tab,
            request: request.clone(),
            context: git_core::DiffContext::Full,
            result: Ok(git_core::FileDiff {
                old_path: Some("app.rs".into()),
                new_path: Some("app.rs".into()),
                binary: false,
                too_large: false,
                encoding: None,
                hunks: text.hunks,
            }),
        });
        let generation = crate::app::tests::open_diff(&app).generation;
        let crate::repo_view::Loadable::Ready(file) = &crate::app::tests::open_diff(&app).diff
        else {
            panic!("diff not loaded");
        };
        let (old, new) = crate::highlight::diff_texts(file);
        let mut partials = Vec::new();
        let cache = std::sync::Mutex::new(crate::highlight::Cache::default());
        let job = crate::highlight::Job::new(theme, None);
        let files = vec![
            crate::highlight::Source::new("app.rs", old, None),
            crate::highlight::Source::new("app.rs", new, None),
        ];
        let start = Instant::now();
        let mut first = None;
        crate::highlight::run(&cache, files, theme, &job.watch(), |o| {
            first.get_or_insert(start.elapsed());
            partials.push(o);
        });
        eprintln!(
            "PERF highlight both sides of the app.rs diff: first block {:?}, complete {:?}",
            first.unwrap(),
            start.elapsed()
        );
        let rows = crate::app::tests::open_diff(&app).rows.len() as f32;
        let row_height = crate::app::tests::open_diff(&app).style.line_height();
        let mut worst = std::time::Duration::ZERO;
        let mut worst_update = std::time::Duration::ZERO;
        for (step, outcomes) in partials.into_iter().enumerate() {
            let mut outcomes = outcomes.into_iter();
            let (old, new) = (outcomes.next().unwrap(), outcomes.next().unwrap());
            let start = Instant::now();
            let _ = app.update(Message::DiffHighlighted {
                tab,
                request: request.clone(),
                generation,
                sides: Arc::new(crate::highlight::DiffSides {
                    theme,
                    language: None,
                    detected: "Rust",
                    old,
                    new,
                }),
            });
            worst_update = worst_update.max(start.elapsed());
            let offset = (step % 7) as f32 / 6.0 * rows * row_height;
            let _ = app.update(Message::DiffScrolled {
                offset,
                height: 800.0,
                width: 1000.0,
            });
            let start = Instant::now();
            let mut sim = simulator(&app);
            let _ = sim.find("app.rs");
            worst = worst.max(start.elapsed());
        }
        eprintln!(
            "PERF partial result update, worst: {worst_update:?}; frame while partial results arrive (view + layout), worst: {worst:?}"
        );
    }

    /// README screenshot (M11-15): a scripted fixture repository with
    /// fictional authors (branches in folders, a merge, tags, a stash, Git
    /// flow, a dirty working tree), Dark, 1600 × 900 at 100 %.
    /// `scripts/screenshot.sh` runs it and copies the PNG to
    /// `docs/screenshots/main-dark.png`.
    #[test]
    #[ignore = "renders the README screenshot; run scripts/screenshot.sh"]
    fn readme_screenshot() {
        use std::process::Command;
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("weather-station");
        std::fs::create_dir_all(&root).unwrap();
        let mut clock = 1_767_000_000_i64;
        let mut git = |args: &[&str], author: (&str, &str)| {
            clock += 5_400;
            let date = format!("@{clock} +0100");
            let output = Command::new("git")
                .args(args)
                .current_dir(&root)
                .env("GIT_AUTHOR_NAME", author.0)
                .env("GIT_AUTHOR_EMAIL", author.1)
                .env("GIT_COMMITTER_NAME", author.0)
                .env("GIT_COMMITTER_EMAIL", author.1)
                .env("GIT_AUTHOR_DATE", &date)
                .env("GIT_COMMITTER_DATE", &date)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .output()
                .unwrap();
            assert!(output.status.success(), "git {args:?}: {output:?}");
        };
        let mia = ("Mia Hollis", "mia@example.org");
        let ravi = ("Ravi Okafor", "ravi@example.org");
        let lena = ("Lena Brandt", "lena@example.org");
        let write = |path: &str, text: &str| {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        type Author<'a> = (&'a str, &'a str);
        let commit = |git: &mut dyn FnMut(&[&str], Author<'_>), message: &str, author| {
            git(&["add", "-A"], author);
            git(&["commit", "-q", "-m", message], author);
        };
        git(&["init", "-q", "-b", "main"], mia);
        git(&["config", "commit.gpgsign", "false"], mia);
        // The commit panel shows this identity, never the machine's.
        git(&["config", "user.name", mia.0], mia);
        git(&["config", "user.email", mia.1], mia);
        write("README.md", "# Weather station\n");
        commit(&mut git, "Initial commit", mia);
        write("src/sensor.rs", "pub fn read() -> f32 { 0.0 }\n");
        commit(&mut git, "Add the sensor driver", mia);
        git(&["tag", "v0.9.0"], mia);
        git(&["switch", "-q", "-c", "develop"], ravi);
        write("src/display.rs", "pub fn show(_: f32) {}\n");
        commit(&mut git, "Show readings on the display", ravi);
        git(&["switch", "-q", "-c", "feature/ui/charts"], lena);
        write("src/chart.rs", "pub fn draw() {}\n");
        commit(&mut git, "Draw temperature charts", lena);
        write("src/chart.rs", "pub fn draw() { /* axes */ }\n");
        commit(&mut git, "Label the chart axes", lena);
        git(&["switch", "-q", "develop"], ravi);
        git(&["switch", "-q", "-c", "feature/alerts"], ravi);
        write("src/alerts.rs", "pub fn check(_: f32) {}\n");
        commit(&mut git, "Warn when it freezes", ravi);
        git(&["switch", "-q", "develop"], ravi);
        git(
            &[
                "merge",
                "-q",
                "--no-ff",
                "feature/alerts",
                "-m",
                "Merge branch 'feature/alerts' into develop",
            ],
            ravi,
        );
        write("src/sensor.rs", "pub fn read() -> f32 { 21.5 }\n");
        commit(&mut git, "Calibrate the sensor", mia);
        git(&["switch", "-q", "-c", "release/1.0.0"], mia);
        write("CHANGELOG.md", "## 1.0.0\n");
        commit(&mut git, "Prepare release 1.0.0", mia);
        git(&["switch", "-q", "main"], mia);
        git(
            &[
                "merge",
                "-q",
                "--no-ff",
                "release/1.0.0",
                "-m",
                "Merge branch 'release/1.0.0'",
            ],
            mia,
        );
        git(&["tag", "v1.0.0"], mia);
        git(&["switch", "-q", "develop"], mia);
        for (key, value) in [
            ("gitflow.branch.master", "main"),
            ("gitflow.branch.develop", "develop"),
            ("gitflow.prefix.feature", "feature/"),
            ("gitflow.prefix.release", "release/"),
            ("gitflow.prefix.hotfix", "hotfix/"),
            ("gitflow.prefix.versiontag", "v"),
        ] {
            git(&["config", key, value], mia);
        }
        // A stash and a dirty working tree.
        write(
            "src/display.rs",
            "pub fn show(value: f32) { let _ = value; }\n",
        );
        git(&["stash", "push", "-q", "-m", "Try a larger font"], mia);
        write("src/sensor.rs", "pub fn read() -> f32 { 21.7 }\n");
        write("notes/ideas.md", "- humidity sensor\n");

        let (mut app, _) = App::new(Vec::new(), None);
        let _ = app.update(crate::app::tests::loaded(crate::session::Session::default()));
        let backend: Arc<dyn git_core::GitBackend> =
            Arc::new(git_core::GixBackend::discover(&root).unwrap());
        let snapshot = crate::snapshot::RepoSnapshot::load(backend.as_ref()).unwrap();
        app.tabs[0].state = crate::app::TabState::Ready(Box::new(crate::repo_view::RepoView::new(
            backend,
            Arc::new(snapshot),
        )));
        let _ = app.update(Message::GitDetected(git_core::GitAvailability::Available(
            git_core::MIN_GIT_VERSION,
        )));
        let _ = app.update(Message::WindowResized(iced::Size::new(1600.0, 900.0)));
        let _ = app.update(Message::WipSelected);
        app.settings.appearance.theme = crate::settings::ThemeChoice::theme("dark");
        snapshot_scaled_sized(&app, "readme-main-dark", 1.0, (1600.0, 900.0));
    }

    /// The toolbar has no panel toggles any more (fix 2026-11-12): right
    /// of "Settings" nothing reacts. Layout only.
    #[test]
    fn toolbar_has_no_panel_toggles() {
        let mut app = crate::app::tests::graph_app();
        let mut sim = simulator(&app);
        let settings = sim.find("Settings").unwrap_or_else(|e| panic!("{e}"));
        let settings = iced_test::selector::Bounded::bounds(&settings);
        sim.point_at(iced::Point::new(
            settings.x + settings.width + 8.0 + 15.0,
            settings.center_y(),
        ));
        let _ = sim.simulate(iced_test::simulator::click());
        for message in sim.into_messages().collect::<Vec<_>>() {
            let _ = app.update(message);
        }
        assert!(app.panes.sidebar_visible && app.panes.panel_visible);
    }

    /// Toolbar (without toggles since the fix of 2026-11-12) with both
    /// panels shown and hidden.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_panel_toggles() {
        let mut app = crate::app::tests::graph_app();
        snapshot(&app, "panels-shown");
        let _ = app.update(crate::app::tests::checked(
            crate::shortcuts::Shortcut::ToggleSidebar,
            false,
        ));
        let _ = app.update(crate::app::tests::checked(
            crate::shortcuts::Shortcut::TogglePanel,
            false,
        ));
        snapshot(&app, "panels-hidden");
    }

    /// A click on the selected file deselects it and closes its view
    /// (M12-01): unstaged, staged and conflicted lists. Layout only.
    #[test]
    fn second_click_on_a_file_deselects_it() {
        use crate::repo_view::CenterView;
        let shown = |app: &App| match &app.tabs[0].view().unwrap().center {
            CenterView::Diff(diff) => Some(diff.request.path.clone()),
            CenterView::Conflict(conflict) => Some(conflict.path.clone()),
            _ => None,
        };
        let mut app = crate::app::tests::staging_app(&["unstaged.txt"], &["staged.txt"]);
        crate::app::tests::with_conflicts(&mut app, &["conflicted.txt"]);
        let _ = app.update(Message::WipSelected);
        for name in ["unstaged.txt", "staged.txt", "conflicted.txt"] {
            // The row's position (the diff header shows the name too).
            let row = {
                let mut sim = simulator(&app);
                let target = sim.find(name).unwrap_or_else(|e| panic!("{e}"));
                iced_test::selector::Bounded::bounds(&target).center()
            };
            let press = |app: &mut App| {
                let messages: Vec<Message> = {
                    let mut sim = simulator(app);
                    sim.point_at(row);
                    let _ = sim.simulate(iced_test::simulator::click());
                    sim.into_messages().collect()
                };
                for message in messages {
                    let _ = app.update(message);
                }
            };
            press(&mut app);
            assert_eq!(shown(&app).as_deref(), Some(name), "{name} opens");
            press(&mut app);
            assert_eq!(shown(&app), None, "{name} closes on the second click");
        }
    }

    /// The sparkle button at the right end of "Committing as …" sends
    /// "Copy commit message prompt" (M12-13); without staged changes it
    /// sends nothing.
    #[test]
    fn prompt_button_click() {
        let clicked = |staged: &[&str]| -> Vec<Message> {
            let mut app = crate::app::tests::staging_app(&["a.txt"], staged);
            let _ = app.update(Message::WipSelected);
            let mut sim = simulator(&app);
            let identity = sim
                .find("Committing as Test Author <author@example.com>")
                .unwrap_or_else(|e| panic!("{e}"));
            let amend = sim
                .find("Amend previous commit")
                .unwrap_or_else(|e| panic!("{e}"));
            let y = iced_test::selector::Bounded::bounds(&identity).center_y();
            let right = {
                let b = iced_test::selector::Bounded::bounds(&amend);
                b.x + b.width
            };
            sim.point_at(iced::Point::new(right - 10.0, y));
            let _ = sim.simulate(iced_test::simulator::click());
            sim.into_messages().collect()
        };
        assert!(clicked(&["s.txt"])
            .iter()
            .any(|m| matches!(m, Message::CopyPromptRequested)));
        assert!(!clicked(&[])
            .iter()
            .any(|m| matches!(m, Message::CopyPromptRequested)));
    }

    /// Commit panel with the sparkle button (M12-13): enabled with staged
    /// changes, disabled without; Dark and Light.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_prompt_button() {
        use crate::settings::ThemeChoice;
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            for (staged, name) in [(&["s.txt"][..], "staged"), (&[][..], "none")] {
                let mut app = crate::app::tests::staging_app(&["a.txt"], staged);
                app.settings.appearance.theme = choice.clone();
                let _ = app.update(Message::WipSelected);
                snapshot(&app, &format!("prompt-button-{name}-{theme}"));
            }
        }
    }

    /// Panel toggles in the status bar (fix 2026-11-12): a click on the
    /// left one and the right one sends the same as Ctrl+J / Ctrl+K and
    /// hides/shows the panels; the start screen has none, the toolbar none.
    #[test]
    fn status_bar_panel_toggles() {
        let version = concat!("v", env!("CARGO_PKG_VERSION"));
        // 0: left toggle, 1: pin button, 2: details panel toggle.
        let press = |app: &mut App, which: u8| {
            let messages: Vec<Message> = {
                let mut sim = simulator(app);
                let label = sim.find(version).unwrap_or_else(|e| panic!("{e}"));
                let bounds = iced_test::selector::Bounded::bounds(&label);
                // Left: 12 px padding + half the 22 px area; right: after the
                // version with the 16 px spacing, the pin first (finding #44).
                let pin = bounds.x + bounds.width + 16.0 + 11.0;
                let x = match which {
                    0 => 12.0 + 11.0,
                    1 => pin,
                    _ => pin + 22.0 + crate::ui::statusbar::PIN_GAP,
                };
                sim.point_at(iced::Point::new(x, bounds.center_y()));
                let _ = sim.simulate(iced_test::simulator::click());
                sim.into_messages().collect()
            };
            for message in messages {
                let _ = app.update(message);
            }
        };
        let click = |app: &mut App, left: bool| press(app, if left { 0 } else { 2 });
        let mut app = crate::app::tests::staging_app(&[], &[]);
        assert!(app.panes.sidebar_visible && app.panes.panel_visible);
        click(&mut app, true);
        assert!(!app.panes.sidebar_visible);
        click(&mut app, false);
        assert!(!app.panes.panel_visible);
        // The shortcut toggles the same state back.
        let _ = app.update(crate::app::tests::checked(
            crate::shortcuts::Shortcut::ToggleSidebar,
            false,
        ));
        assert!(app.panes.sidebar_visible);
        click(&mut app, false);
        assert!(app.panes.panel_visible);
        // The pin moves the panel to the bottom and back.
        press(&mut app, 1);
        assert_eq!(
            app.panes.position,
            crate::panes::DetailsPanelPosition::Bottom
        );
        click(&mut app, false);
        assert!(!app.panes.panel_visible, "the toggle stays in place");
        click(&mut app, false);
        press(&mut app, 1);
        assert_eq!(
            app.panes.position,
            crate::panes::DetailsPanelPosition::Right
        );
        // Start screen: the left end of the status bar is not a toggle.
        let (mut start, _) = App::new(Vec::new(), None);
        let before = start.panes.sidebar_visible;
        click(&mut start, true);
        assert_eq!(start.panes.sidebar_visible, before);
        // Icons follow the state.
        use crate::icons::Icon;
        use crate::ui::statusbar::{panel_icon, PanelToggle};
        assert_eq!(panel_icon(PanelToggle::Left, true), Icon::PanelLeftFilled);
        assert_eq!(
            panel_icon(
                PanelToggle::Details(crate::panes::DetailsPanelPosition::Right),
                false
            ),
            Icon::PanelRight
        );
    }

    /// Status bar with both panels shown, both hidden and mixed; three
    /// themes at 100 % and the toggles at 125 % (fix 2026-11-12).
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_status_bar_toggles() {
        use crate::settings::ThemeChoice;
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            for (left, right, name) in [
                (true, true, "both"),
                (false, false, "none"),
                (false, true, "mixed"),
            ] {
                let mut app = crate::app::tests::staging_app(&[], &[]);
                app.settings.appearance.theme = choice.clone();
                app.panes.sidebar_visible = true;
                app.panes.panel_visible = true;
                if !left {
                    app.panes.toggle_sidebar();
                }
                if !right {
                    app.panes.toggle_panel();
                }
                snapshot(&app, &format!("statusbar-{name}-{theme}"));
            }
            let mut app = crate::app::tests::staging_app(&[], &[]);
            app.settings.appearance.theme = choice.clone();
            let _ = app.update(Message::SystemScaleChanged(1.25));
            snapshot_scaled(&app, &format!("statusbar-125-{theme}"), 1.25);
        }
    }

    /// ADR-057: at the window minimum width the language item with the
    /// longest language name keeps to its width, and it, the branch label,
    /// the commit count, the version and the pin and panel buttons do not
    /// overlap and stay inside the window; both details panel positions.
    /// Layout only, part of every run.
    #[test]
    fn status_bar_language_item_fits_at_the_window_minimum() {
        use crate::panes::DetailsPanelPosition;
        use crate::ui::statusbar::{LANGUAGE_MAX_WIDTH, PIN_GAP};
        let min = crate::layout::window_min();
        let size = (min.width, min.height);
        let longest = *crate::highlight::language_names()
            .iter()
            .max_by_key(|name| name.chars().count())
            .unwrap();
        for position in [DetailsPanelPosition::Right, DetailsPanelPosition::Bottom] {
            let mut app = crate::app::tests::godot_diff_app(
                crate::settings::ThemeChoice::theme("dark"),
                Some(longest),
            );
            app.panes.set_position(position);
            let _ = app.update(Message::WindowResized(min));
            let view = app.tabs[0].view().unwrap();
            let branch = format!("branch: {}", view.snapshot.branch_label());
            let label = bounds_in(&app, size, longest);
            let branch = bounds_in(&app, size, &branch);
            let version = bounds_in(&app, size, concat!("v", env!("CARGO_PKG_VERSION")));
            // Clipped to the item (button padding 4 on both sides).
            assert!(label.width <= LANGUAGE_MAX_WIDTH - 8.0, "{label:?}");
            assert!(label.x > 0.0);
            // 16 px row spacing between the item and the branch label.
            assert!(
                label.x + label.width + 4.0 + 16.0 <= branch.x + 0.5,
                "{label:?} {branch:?}"
            );
            assert!(branch.x + branch.width < version.x);
            // Pin and panel toggle (22 px each) and the 12 px padding.
            let end = version.x + version.width + 16.0 + 22.0 + PIN_GAP + 22.0 + 12.0;
            assert!(end <= min.width + 0.5, "{end} > {}", min.width);
            assert!(label.y >= min.height - 26.0, "in the status bar");
        }
    }

    /// ADR-057: the status bar with the language item (Dark and Light,
    /// 100 % and 125 %) on the INI-colored diff of `project.godot`, and the
    /// language list filtered by "ini".
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_language_mode() {
        use crate::settings::ThemeChoice;
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            let mut app = crate::app::tests::godot_diff_app(choice.clone(), Some("INI"));
            snapshot(&app, &format!("language-statusbar-{theme}"));
            let _ = app.update(Message::SystemScaleChanged(1.25));
            snapshot_scaled(&app, &format!("language-statusbar-125-{theme}"), 1.25);
            let mut app = crate::app::tests::godot_diff_app(choice, None);
            snapshot(&app, &format!("language-auto-{theme}"));
            let _ = app.update(crate::app::tests::checked(
                crate::shortcuts::Shortcut::SelectLanguage,
                false,
            ));
            snapshot(&app, &format!("language-palette-{theme}"));
            let _ = app.update(Message::PaletteQuery("ini".into()));
            snapshot(&app, &format!("language-palette-ini-{theme}"));
        }
        // The longest name at the window minimum.
        let min = crate::layout::window_min();
        let longest = *crate::highlight::language_names()
            .iter()
            .max_by_key(|name| name.chars().count())
            .unwrap();
        let mut app = crate::app::tests::godot_diff_app(ThemeChoice::theme("dark"), Some(longest));
        let _ = app.update(Message::WindowResized(min));
        snapshot_sized(&app, "language-minimum-longest", (min.width, min.height));
    }

    /// Finding #44: the status bar with the pin button in both positions
    /// and the layout with the details panel at the bottom (graph above,
    /// panel below), Dark and Light.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_details_panel_position() {
        use crate::panes::DetailsPanelPosition;
        use crate::settings::ThemeChoice;
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            for position in [DetailsPanelPosition::Right, DetailsPanelPosition::Bottom] {
                let mut app = crate::app::tests::graph_app();
                app.settings.appearance.theme = choice.clone();
                app.panes.set_position(position);
                let _ = app.update(Message::CommitSelected("f".into()));
                let name = match position {
                    DetailsPanelPosition::Right => "right",
                    DetailsPanelPosition::Bottom => "bottom",
                };
                snapshot(&app, &format!("panel-position-{name}-{theme}"));
                app.panes.toggle_panel();
                snapshot(&app, &format!("panel-position-{name}-hidden-{theme}"));
            }
        }
    }

    /// Finding #44: the details panel at the bottom: staging empty and
    /// with files in both lists (Path and Tree), commit details, a merge
    /// in progress, a stash and several commits; Dark and Light.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_bottom_layouts() {
        use crate::panes::DetailsPanelPosition;
        use crate::settings::ThemeChoice;
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            let bottom = |app: &mut App| {
                app.settings.appearance.theme = choice.clone();
                app.panes.set_position(DetailsPanelPosition::Bottom);
            };
            let mut app = crate::app::tests::staging_app(&[], &[]);
            bottom(&mut app);
            let _ = app.update(Message::WipSelected);
            snapshot(&app, &format!("bottom-staging-empty-{theme}"));
            let mut app = crate::app::tests::staging_app(
                &["src/main.rs", "src/ui/panel.rs", "README.md", "new.txt"],
                &["Cargo.toml", "src/lib.rs"],
            );
            bottom(&mut app);
            let _ = app.update(Message::WipSelected);
            snapshot(&app, &format!("bottom-staging-files-{theme}"));
            let _ = app.update(Message::FileListTreeChosen(true));
            snapshot(&app, &format!("bottom-staging-tree-{theme}"));
            let _ = app.update(Message::FileListTreeChosen(false));

            let mut app = file_list_app();
            bottom(&mut app);
            snapshot(&app, &format!("bottom-details-{theme}"));

            let mut app = crate::app::tests::app_in_state(git_core::RepoState::Merging {
                merge_head: "x".into(),
            });
            bottom(&mut app);
            let view = app.tabs[0].view_mut().unwrap();
            let tree = git_core::WorkingTree {
                unstaged: vec![git_core::FileChange {
                    path: "src/main.rs".into(),
                    kind: git_core::ChangeKind::Conflicted,
                    old_path: None,
                }],
                staged: vec![git_core::FileChange {
                    path: "README.md".into(),
                    kind: git_core::ChangeKind::Modified,
                    old_path: None,
                }],
            };
            view.operation_conflicts = Some(["src/main.rs".to_owned()].into());
            view.snapshot = std::sync::Arc::new(view.snapshot.with_working_tree(tree).unwrap());
            let _ = app.update(Message::WipSelected);
            snapshot(&app, &format!("bottom-merge-{theme}"));

            let mut app = crate::app::tests::graph_app();
            bottom(&mut app);
            let tab = app.tabs[0].id;
            let _ = app.update(Message::CommitSelected("s".into()));
            let file = |path: &str| git_core::FileChange {
                path: path.into(),
                kind: git_core::ChangeKind::Modified,
                old_path: None,
            };
            let _ = app.update(Message::CommitChangesLoaded {
                tab,
                id: "s".into(),
                result: Ok(crate::repo_view::DetailChanges::Stash(
                    git_core::StashChanges {
                        staged: vec![file("a.rs")],
                        unstaged: vec![file("b.rs"), file("c.rs")],
                        untracked: Vec::new(),
                    },
                )),
            });
            snapshot(&app, &format!("bottom-stash-{theme}"));
            let _ = app.update(Message::CommitSelected("a".into()));
            {
                let view = app.tabs[0].view_mut().unwrap();
                let order: Vec<String> =
                    view.snapshot.commits.iter().map(|c| c.id.clone()).collect();
                let order: Vec<&str> = order.iter().map(String::as_str).collect();
                view.multi.toggle("a", &order);
                view.multi.toggle("e", &order);
            }
            snapshot(&app, &format!("bottom-multi-{theme}"));
        }
    }

    /// The hover shows only "Stage File" / "Unstage File" (M12-01).
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_file_row_hover() {
        let mut app = crate::app::tests::staging_app(&["unstaged.txt"], &["staged.txt"]);
        let _ = app.update(Message::WipSelected);
        for (name, label) in [
            ("unstaged.txt", "Stage File"),
            ("staged.txt", "Unstage File"),
        ] {
            let mut sim = simulator(&app);
            let target = sim.find(name).unwrap_or_else(|e| panic!("{e}"));
            sim.point_at(iced_test::selector::Bounded::bounds(&target).center());
            let _ = sim.simulate([iced::Event::Mouse(iced::mouse::Event::CursorMoved {
                position: iced_test::selector::Bounded::bounds(&target).center(),
            })]);
            assert!(sim.find(label).is_ok(), "{label} on hover");
            assert!(sim.find("Discard").is_err(), "no Discard on hover");
            let snap = sim.snapshot(&app.theme()).unwrap();
            assert!(snap
                .matches_image(fresh_path(&format!("file-hover-{label}")))
                .unwrap());
        }
    }

    /// Toasts of every kind in all themes (M12-02), one with details open.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_toasts() {
        use crate::settings::ThemeChoice;
        use crate::toast::{Toast, ToastKind};
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            let mut app = crate::app::tests::graph_app();
            app.settings.appearance.theme = choice.clone();
            app.toasts.push(Toast::git_error(
                "Push failed",
                "fatal: unable to access 'https://example.org/r.git/': Could not resolve host: example.org",
            ));
            app.toasts.push(Toast::new(
                ToastKind::Warning,
                "Submodules are out of date",
                "Two submodules point to other commits than recorded.",
            ));
            app.toasts.push(Toast::new(
                ToastKind::Success,
                "Pushed",
                "main → origin/main",
            ));
            snapshot(&app, &format!("toasts-{theme}"));
            let id = app.toasts.list[0].id;
            let _ = app.update(Message::ToastDetails(id));
            snapshot(&app, &format!("toasts-details-{theme}"));
            // A fourth pushes out the oldest; info is the remaining kind.
            app.toasts
                .push(Toast::new(ToastKind::Info, "Fetched", "3 new commits"));
            snapshot(&app, &format!("toasts-info-{theme}"));
        }
    }

    /// Preferences modal (M11-13): a click on the backdrop does not close it,
    /// and the app behind is out of reach for focus and Tab. Layout only.
    #[test]
    fn preferences_modal_keeps_clicks_and_focus_inside() {
        let mut app = crate::app::tests::sidebar_app();
        // Without the modal the sidebar filter is reachable.
        assert!(simulator(&app)
            .find(iced::widget::Id::new(crate::app::FILTER_INPUT))
            .is_ok());
        let _ = app.update(Message::SettingsOpened);
        let mut sim = simulator(&app);
        assert!(
            sim.find(iced::widget::Id::new(crate::app::FILTER_INPUT))
                .is_err(),
            "the app behind is skipped by focus operations"
        );
        assert!(sim
            .find(iced::widget::Id::new(crate::ui::settings::AUTO_FETCH_INPUT))
            .is_ok());
        // A click on the backdrop (left of the 960 px dialog) does nothing.
        sim.point_at(iced::Point::new(40.0, 400.0));
        let _ = sim.simulate(iced_test::simulator::click());
        let messages: Vec<Message> = sim.into_messages().collect();
        assert!(
            !messages
                .iter()
                .any(|m| matches!(m, Message::SettingsClosed | Message::RefClicked { .. })),
            "{messages:?}"
        );
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
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
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
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            for (zoom, size) in [(100, SIZE), (200, (SIZE.0 / 2.0, SIZE.1 / 2.0))] {
                snapshot_sized(&app, &format!("graph-lines-{theme}-{zoom}"), size);
            }
        }
    }

    /// Resized graph columns (M11-02): a Graph column of one lane puts every
    /// node on one line (M15-05); wider Branch/Tag and Author columns; both
    /// themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_graph_columns() {
        let mut app = crate::app::tests::graph_app();
        for (light, theme) in [(false, "dark"), (true, "light")] {
            app.settings.appearance.theme = if light {
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
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

    /// Narrow Graph column (M15-05): the fixture graph and the Git flow
    /// fixture at full, half and minimum width; overflowing lanes' nodes in
    /// the last slot; both themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_graph_overflow() {
        let (_repo, flow) = crate::snapshot::flow_tests::classic_snapshot();
        for (light, theme) in [(false, "dark"), (true, "light")] {
            for fixture in ["graph", "flow"] {
                let mut app = crate::app::tests::graph_app();
                app.settings.appearance.theme = if light {
                    crate::settings::ThemeChoice::theme("light")
                } else {
                    crate::settings::ThemeChoice::theme("dark")
                };
                if fixture == "flow" {
                    let view = app.tabs[0].view_mut().unwrap();
                    view.snapshot = std::sync::Arc::new(flow.with_flow_layout(true).unwrap());
                }
                let lanes = app.tabs[0].view().unwrap().snapshot.layout.max_width;
                let full = crate::ui::graph::lanes_width(lanes);
                for (width, name) in [
                    (full, "full"),
                    (full / 2.0, "half"),
                    (crate::ui::MIN_GRAPH_WIDTH, "min"),
                ] {
                    app.settings.appearance.graph_columns.graph = Some(width);
                    snapshot_sized(
                        &app,
                        &format!("graph-overflow-{fixture}-{name}-{theme}"),
                        (1200.0, 520.0),
                    );
                }
            }
        }
    }

    /// Column widths and horizontal scrolling (M11-16): a table wider than
    /// the window scrolled to the middle, and one narrower than the window
    /// (the message fills); both themes.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_graph_scrolling() {
        use crate::settings::ThemeChoice;
        for (choice, theme) in [
            (ThemeChoice::theme("dark"), "dark"),
            (ThemeChoice::theme("light"), "light"),
        ] {
            let mut app = crate::app::tests::graph_app();
            app.settings.appearance.theme = choice.clone();
            snapshot(&app, &format!("graph-scroll-fill-{theme}"));
            app.settings.appearance.graph_columns.message = 900.0;
            app.settings.appearance.graph_columns.author = 260.0;
            let _ = app.update(Message::GraphHScrolled {
                offset: 280.0,
                max: 560.0,
            });
            snapshot(&app, &format!("graph-scroll-middle-{theme}"));
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
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            app.settings.appearance.theme = choice.clone();
            start.settings.appearance.theme = choice.clone();
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
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
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
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
            };
            app.settings.appearance.theme = choice.clone();
            none.settings.appearance.theme = choice.clone();
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
                crate::settings::ThemeChoice::theme("light")
            } else {
                crate::settings::ThemeChoice::theme("dark")
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

    // ---- Finding #48: pane dividers and the scrollbars next to them ----

    /// Scroll state of one scrollable: its bounds, its content and the
    /// scroll offset.
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Scrolled {
        bounds: iced::Rectangle,
        content: iced::Rectangle,
        offset: iced::Vector,
    }

    impl Scrolled {
        fn overflow(&self) -> iced::Size {
            iced::Size::new(
                (self.content.width - self.bounds.width).max(0.0),
                (self.content.height - self.bounds.height).max(0.0),
            )
        }
    }

    /// All scrollables of the simulated view, in tree order.
    fn scrolled(sim: &mut Simulator<'_, Message>) -> Vec<Scrolled> {
        use iced_test::selector::Candidate;
        let mut out = Vec::new();
        let _ = sim.find(|candidate: Candidate<'_>| -> Option<()> {
            if let Candidate::Scrollable {
                bounds,
                content_bounds,
                translation,
                ..
            } = candidate
            {
                out.push(Scrolled {
                    bounds,
                    content: content_bounds,
                    offset: translation,
                });
            }
            None
        });
        out
    }

    /// Bounds of the pane `kind` in the simulated view.
    fn pane_bounds(
        sim: &mut Simulator<'_, Message>,
        kind: crate::panes::PaneKind,
    ) -> iced::Rectangle {
        use iced_test::selector::Candidate;
        let id = crate::ui::pane_id(kind);
        sim.find(|candidate: Candidate<'_>| match candidate {
            Candidate::Container {
                id: Some(found),
                bounds,
                ..
            } if *found == id => Some(bounds),
            _ => None,
        })
        .unwrap_or_else(|e| panic!("no pane {kind:?}: {e}"))
    }

    /// Moves the cursor along `path` with the left button pressed at the
    /// first point and released at the last one.
    fn press_along(sim: &mut Simulator<'_, Message>, path: &[iced::Point]) {
        use iced::mouse::{Button, Event as Mouse};
        use iced::Event;
        for (i, &position) in path.iter().enumerate() {
            sim.point_at(position);
            let mut events = vec![Event::Mouse(Mouse::CursorMoved { position })];
            if i == 0 {
                events.push(Event::Mouse(Mouse::ButtonPressed(Button::Left)));
            }
            if i + 1 == path.len() {
                events.push(Event::Mouse(Mouse::ButtonReleased(Button::Left)));
            }
            let _ = sim.simulate(events);
        }
    }

    /// A drag from `start`: up, down, sideways and back, ending `end`
    /// away from the start.
    fn divider_path(start: iced::Point, end: iced::Vector) -> Vec<iced::Point> {
        [
            iced::Vector::new(0.0, 0.0),
            iced::Vector::new(1.0, 1.0),
            iced::Vector::new(40.0, -150.0),
            iced::Vector::new(-40.0, 150.0),
            iced::Vector::new(90.0, 60.0),
            iced::Vector::new(-90.0, -60.0),
            end,
        ]
        .into_iter()
        .map(|d| start + d)
        .collect()
    }

    /// Scrolls every scrollable with overflow a third of the way with the
    /// wheel; returns the states afterwards.
    fn scroll_all(sim: &mut Simulator<'_, Message>) -> Vec<Scrolled> {
        use iced::mouse::{Event as Mouse, ScrollDelta};
        use iced::Event;
        for s in scrolled(sim) {
            let overflow = s.overflow();
            if overflow.width < 1.0 && overflow.height < 1.0 {
                continue;
            }
            let at = s.bounds.center();
            sim.point_at(at);
            let _ = sim.simulate(vec![
                Event::Mouse(Mouse::CursorMoved { position: at }),
                Event::Mouse(Mouse::WheelScrolled {
                    delta: ScrollDelta::Pixels {
                        x: -overflow.width / 3.0,
                        y: -overflow.height / 3.0,
                    },
                }),
                // Ends the wheel transaction of the scrollable.
                Event::Mouse(Mouse::ButtonReleased(iced::mouse::Button::Left)),
            ]);
        }
        scrolled(sim)
    }

    /// Graph of 400 commits, 60 local branches, 60 unstaged and 60 staged
    /// files (WIP selected) and a message column wider than the graph, so
    /// the branch tree, the graph (both ways) and the staging lists scroll.
    fn divider_app(position: crate::panes::DetailsPanelPosition) -> App {
        let ids: Vec<String> = (0..400).map(|i| format!("c{i:03}")).collect();
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        let mut app = ready_app(&ids);
        let _ = app.update(Message::GitDetected(git_core::GitAvailability::Available(
            git_core::MIN_GIT_VERSION,
        )));
        let change = |path: String| git_core::FileChange {
            path,
            kind: git_core::ChangeKind::Modified,
            old_path: None,
        };
        let view = app.tabs[0].view_mut().unwrap();
        let mut snapshot = view
            .snapshot
            .with_working_tree(git_core::WorkingTree {
                unstaged: (0..60).map(|i| change(format!("src/u{i:02}.rs"))).collect(),
                staged: (0..60).map(|i| change(format!("src/s{i:02}.rs"))).collect(),
            })
            .unwrap();
        for i in 0..60 {
            snapshot.refs.push(git_core::Reference {
                name: format!("feature/branch-{i:02}"),
                kind: git_core::RefKind::LocalBranch,
                target: "c000".into(),
                is_head: false,
                upstream: None,
                ahead_behind: None,
            });
        }
        view.snapshot = std::sync::Arc::new(snapshot);
        app.settings
            .appearance
            .graph_columns
            .set(crate::graph_columns::ColumnId::Message, 1600.0);
        app.panes.set_position(position);
        let _ = app.update(Message::WindowResized(iced::Size::new(SIZE.0, SIZE.1)));
        let _ = app.update(Message::WipSelected);
        app
    }

    /// Finding #48: a drag on a pane divider (or the staging splitter)
    /// starting at `start` leaves every scroll position alone and resizes;
    /// `resized` checks the app after the drag's messages.
    fn assert_divider_drag(
        name: &str,
        app: &App,
        start: iced::Point,
        end: iced::Vector,
        resized: impl Fn(&App) -> bool,
    ) {
        let mut sim = simulator(app);
        let before = scroll_all(&mut sim);
        press_along(&mut sim, &divider_path(start, end));
        let after = scrolled(&mut sim);
        let messages: Vec<Message> = sim.into_messages().collect();
        assert_eq!(before.len(), after.len(), "{name}");
        for (b, a) in before.iter().zip(&after) {
            assert_eq!(
                b.offset, a.offset,
                "{name}: the scrollable at {:?} moved",
                b.bounds
            );
        }
        let mut app = clone_layout(app);
        let mut any = false;
        for message in messages {
            if matches!(
                message,
                Message::PaneResized(_)
                    | Message::StagingSplitStart
                    | Message::StagingSplitDragged { .. }
                    | Message::StagingSplitEnd
            ) {
                any = true;
                let _ = app.update(message);
            }
        }
        assert!(any, "{name}: no resize from {start:?}");
        assert!(resized(&app), "{name}: not resized as expected");
    }

    /// A fresh app with the panes of `app` (ratios, position) for replaying
    /// resize messages.
    fn clone_layout(app: &App) -> App {
        let mut copy = divider_app(app.panes.position);
        copy.panes.set_ratios(app.panes.ratios);
        copy
    }

    /// Finding #48: a press on the scrollbar of `pane`'s scrollable at
    /// `point` (thumb or track, right next to a divider) still scrolls.
    fn assert_scrollbar_works(
        name: &str,
        app: &App,
        at: impl Fn(&Scrolled) -> iced::Point,
        pick: impl Fn(&Scrolled) -> bool,
        delta: iced::Vector,
    ) {
        let mut sim = simulator(app);
        let before = scroll_all(&mut sim);
        let index = before
            .iter()
            .position(&pick)
            .unwrap_or_else(|| panic!("{name}: no such scrollable in {before:?}"));
        let start = at(&before[index]);
        press_along(&mut sim, &[start, start + delta]);
        let after = scrolled(&mut sim);
        assert_ne!(
            before[index].offset, after[index].offset,
            "{name}: the scrollbar at {start:?} did not scroll"
        );
    }

    /// Finding #48: dragging any divider keeps the scroll positions of the
    /// branch tree, the graph (both ways), the diff and the staging lists;
    /// the panes resize. Before the fix the press also grabbed the
    /// scrollbar under the divider's grab area. Layout only.
    #[test]
    fn dividers_leave_neighbouring_scrollbars_alone() {
        use crate::panes::{DetailsPanelPosition, PaneKind};
        use iced::{Point, Vector};
        let reach = crate::layout::DIVIDER_REACH;
        // Right: Sidebar | Graph | Panel.
        let app = divider_app(DetailsPanelPosition::Right);
        let ratios = app.panes.ratios;
        let mut sim = simulator(&app);
        let sidebar = pane_bounds(&mut sim, PaneKind::Sidebar);
        let center = pane_bounds(&mut sim, PaneKind::Center);
        let all = scrolled(&mut sim);
        drop(sim);
        assert!(
            all.iter().filter(|s| s.overflow().height > 0.0).count() >= 4,
            "tree, graph and both lists scroll: {all:?}"
        );
        assert!(all.iter().any(|s| s.overflow().width > 0.0), "{all:?}");
        let mid = sidebar.center_y();
        let hbar = center.y + center.height - 5.0;
        for (name, start) in [
            (
                "sidebar edge",
                Point::new(sidebar.x + sidebar.width - 1.0, mid),
            ),
            (
                "sidebar edge, outer",
                Point::new(sidebar.x + sidebar.width - reach + 0.5, mid),
            ),
            (
                "graph left edge at the hbar",
                Point::new(center.x + 1.0, hbar),
            ),
        ] {
            assert_divider_drag(name, &app, start, Vector::new(25.0, 20.0), |a| {
                a.panes.ratios.sidebar > ratios.sidebar
            });
        }
        for (name, start) in [
            (
                "graph right edge",
                Point::new(center.x + center.width - 1.0, mid),
            ),
            (
                "graph right edge, outer",
                Point::new(center.x + center.width - reach + 0.5, mid),
            ),
            (
                "graph right edge at the hbar",
                Point::new(center.x + center.width - 1.0, hbar),
            ),
            (
                "panel left edge",
                Point::new(center.x + center.width + 2.0, mid),
            ),
        ] {
            assert_divider_drag(name, &app, start, Vector::new(-25.0, 20.0), |a| {
                a.panes.ratios.center < ratios.center
            });
        }
        // Next to the dividers the scrollbars still work: the thumb and
        // the track of the branch tree and the graph.
        let right_of = |pane: iced::Rectangle| {
            move |s: &Scrolled| {
                s.overflow().height > 0.0
                    && (s.bounds.x + s.bounds.width - (pane.x + pane.width)).abs() < 1.0
            }
        };
        let thumb = |s: &Scrolled| {
            let share = s.bounds.height / s.content.height;
            Point::new(
                s.bounds.x + s.bounds.width - 6.0,
                s.bounds.y + (s.offset.y + s.bounds.height / 2.0) * share,
            )
        };
        let track = |s: &Scrolled| {
            Point::new(
                s.bounds.x + s.bounds.width - 6.0,
                s.bounds.y + s.bounds.height - 4.0,
            )
        };
        for pane in [sidebar, center] {
            assert_scrollbar_works("thumb", &app, thumb, right_of(pane), Vector::new(0.0, 60.0));
            assert_scrollbar_works("track", &app, track, right_of(pane), Vector::new(0.0, 0.0));
        }
        let hbar_of = |s: &Scrolled| s.overflow().width > 0.0;
        assert_scrollbar_works(
            "graph hbar near the left divider",
            &app,
            |s| Point::new(s.bounds.x + 6.0, s.bounds.center_y()),
            hbar_of,
            Vector::new(0.0, 0.0),
        );

        // Bottom: Sidebar | (Graph / Panel), the staging splitter.
        let app = divider_app(DetailsPanelPosition::Bottom);
        let ratios = app.panes.ratios;
        let mut sim = simulator(&app);
        let center = pane_bounds(&mut sim, PaneKind::Center);
        let panel = pane_bounds(&mut sim, PaneKind::Panel);
        let all = scrolled(&mut sim);
        drop(sim);
        let hbar = all
            .iter()
            .find(|s| s.overflow().width > 0.0)
            .copied()
            .expect("graph hbar");
        let bottom = center.y + center.height - 1.0;
        for (name, start) in [
            (
                "graph bottom edge at the hbar",
                Point::new(hbar.bounds.center_x(), bottom),
            ),
            (
                "graph bottom edge at the vertical bar",
                Point::new(center.x + center.width - 5.0, bottom),
            ),
            (
                "graph bottom edge, outer",
                Point::new(
                    hbar.bounds.center_x(),
                    center.y + center.height - reach + 0.5,
                ),
            ),
            (
                "panel top edge",
                Point::new(panel.center_x(), panel.y + 1.0),
            ),
        ] {
            assert_divider_drag(name, &app, start, Vector::new(15.0, -30.0), |a| {
                a.panes.ratios.bottom < ratios.bottom
            });
        }
        assert_divider_drag(
            "sidebar edge (bottom)",
            &app,
            Point::new(center.x - 2.0, panel.center_y()),
            Vector::new(25.0, 0.0),
            |a| a.panes.ratios.sidebar > ratios.sidebar,
        );
        assert_scrollbar_works(
            "graph hbar above the divider",
            &app,
            |s| Point::new(s.bounds.x + s.bounds.width - 20.0, s.bounds.y + 4.0),
            hbar_of,
            Vector::new(0.0, 0.0),
        );
        // The staging splitter between the lists and the message, with
        // room to move.
        let mut app = app;
        app.panes.set_ratios(crate::panes::PaneRatios {
            bottom: 0.3,
            ..app.panes.ratios
        });
        let mut sim = simulator(&app);
        let label = sim
            .find("Commit Message")
            .map(|t| iced_test::selector::Bounded::bounds(&t))
            .expect("label");
        drop(sim);
        let handle = Point::new(
            panel.center_x() - 100.0,
            label.y - crate::ui::split_handle::HEIGHT / 2.0,
        );
        let staging = app.panes.ratios.staging;
        assert_divider_drag(
            "staging splitter",
            &app,
            handle,
            Vector::new(0.0, 30.0),
            |a| a.panes.ratios.staging > staging,
        );
    }

    /// Finding #48 for the diff: its horizontal bar at the bottom of the
    /// center and its vertical bar at the right keep their positions.
    /// Layout only.
    #[test]
    fn dividers_leave_the_diff_scrollbars_alone() {
        use crate::panes::{DetailsPanelPosition, PaneKind};
        use iced::{Point, Vector};
        for position in [DetailsPanelPosition::Right, DetailsPanelPosition::Bottom] {
            let mut app = crate::app::tests::wide_diff_app(crate::settings::DiffMode::Inline);
            app.panes.set_position(position);
            let _ = app.update(Message::WindowResized(iced::Size::new(SIZE.0, SIZE.1)));
            let ratios = app.panes.ratios;
            let mut sim = simulator(&app);
            let center = pane_bounds(&mut sim, PaneKind::Center);
            let all = scrolled(&mut sim);
            drop(sim);
            let hbar = all
                .iter()
                .find(|s| s.overflow().width > 0.0)
                .copied()
                .unwrap_or_else(|| panic!("{position:?}: diff hbar in {all:?}"));
            let start = match position {
                DetailsPanelPosition::Right => {
                    Point::new(center.x + center.width - 1.0, center.center_y())
                }
                DetailsPanelPosition::Bottom => {
                    Point::new(hbar.bounds.center_x(), center.y + center.height - 1.0)
                }
            };
            let name = format!("diff {position:?}");
            let mut sim = simulator(&app);
            let before = scroll_all(&mut sim);
            press_along(&mut sim, &divider_path(start, Vector::new(-20.0, -20.0)));
            let after = scrolled(&mut sim);
            let messages: Vec<Message> = sim.into_messages().collect();
            assert_eq!(before, after, "{name}");
            let mut resized = false;
            for message in messages {
                if let Message::PaneResized(_) = message {
                    let _ = app.update(message);
                    resized = true;
                }
            }
            assert!(resized, "{name}");
            assert_ne!(app.panes.ratios, ratios, "{name}");
        }
    }

    /// Renders `app` with the cursor at `cursor` (hover states).
    fn snapshot_at(app: &App, name: &str, size: (f32, f32), cursor: iced::Point) -> PathBuf {
        let path = fresh_path(name);
        let mut sim = Simulator::with_size(settings(), size, app.view());
        sim.point_at(cursor);
        let _ = sim.simulate(vec![iced::Event::Mouse(iced::mouse::Event::CursorMoved {
            position: cursor,
        })]);
        let snap = sim.snapshot(&app.theme()).unwrap();
        assert!(snap.matches_image(&path).unwrap());
        path
    }

    /// Finding #47: the Unstaged and Staged lists in the frame of the
    /// commit details' file list, at the right and at the bottom; empty,
    /// a few and many files; a selected row and a hovered one; Dark and
    /// Light.
    #[test]
    #[ignore = "renders the UI; run with --ignored"]
    fn ui_check_staging_frames() {
        use crate::panes::DetailsPanelPosition;
        let names = |prefix: &str, n: usize| -> Vec<String> {
            (0..n).map(|i| format!("src/{prefix}{i:02}.rs")).collect()
        };
        for (choice, theme) in [
            (crate::settings::ThemeChoice::theme("dark"), "dark"),
            (crate::settings::ThemeChoice::theme("light"), "light"),
        ] {
            for (position, place) in [
                (DetailsPanelPosition::Right, "right"),
                (DetailsPanelPosition::Bottom, "bottom"),
            ] {
                for (amount, unstaged, staged) in [("empty", 0, 0), ("few", 3, 2), ("many", 40, 30)]
                {
                    let unstaged = names("u", unstaged);
                    let staged = names("s", staged);
                    let u: Vec<&str> = unstaged.iter().map(String::as_str).collect();
                    let st: Vec<&str> = staged.iter().map(String::as_str).collect();
                    let mut app = crate::app::tests::staging_app(&u, &st);
                    app.settings.appearance.theme = choice.clone();
                    app.panes.set_position(position);
                    let _ = app.update(Message::WindowResized(iced::Size::new(SIZE.0, SIZE.1)));
                    let _ = app.update(Message::WipSelected);
                    if let Some(first) = app.tabs[0]
                        .view()
                        .unwrap()
                        .snapshot
                        .working_tree
                        .unstaged
                        .first()
                        .cloned()
                    {
                        let _ = app.update(Message::DiffOpened(crate::repo_view::diff_request(
                            &first, false,
                        )));
                    }
                    let name = format!("staging-frame-{place}-{amount}-{theme}");
                    // Hover the first staged row, if any.
                    let cursor = st
                        .first()
                        .map(|label| {
                            let b = bounds_in(&app, SIZE, label);
                            iced::Point::new(b.x + b.width / 2.0, b.center_y())
                        })
                        .unwrap_or(iced::Point::ORIGIN);
                    snapshot_at(&app, &name, SIZE, cursor);
                }
            }
        }
    }
}
