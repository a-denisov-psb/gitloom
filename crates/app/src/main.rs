// Release builds on Windows use the GUI subsystem, so no console window opens
// next to Gitloom and a terminal returns to the prompt right away (M6-16).
// Debug builds keep the console for panics and logs. Helper modes (askpass,
// Git editor) still use the stdio handles Git passes in.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod about;
mod actions;
mod app;
mod app_icon;
mod askpass;
mod avatars;
mod branch_tree;
mod clone_dialog;
mod conflict;
mod crash;
mod diff_blocks;
mod diff_hscroll;
mod diff_wrap;
mod editor_style;
mod fetch;
mod file_history;
mod format;
mod fuzzy;
mod graph_columns;
mod highlight;
mod icons;
mod launch;
mod menus;
mod nav;
mod operation;
mod overlay;
mod palette;
mod panes;
mod rebase_panel;
mod ref_filter;
mod ref_selection;
mod repo_view;
mod search;
mod selection;
mod session;
mod settings;
mod settings_registry;
mod shortcuts;
mod snapshot;
mod stash_template;
/// Temporary repositories built with the `git` CLI (git-core's test support).
#[cfg(test)]
#[path = "../../git-core/tests/support/mod.rs"]
mod test_repo;
mod theme;
mod toast;
mod ui;
#[cfg(test)]
mod ui_check;
mod undo;
mod update;

use std::path::PathBuf;

use app::App;

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    // Started by Git as sequence or message editor (ADR-021): no UI.
    if let Some((kind, file)) = git_core::editor_call(&args) {
        let dir = std::env::var_os(git_core::EDITOR_DIR_ENV).map(PathBuf::from);
        std::process::exit(git_core::editor_main(kind, &file, dir.as_deref()));
    }
    // Started by Git or SSH to ask for credentials (ADR-019).
    let marker = std::env::var_os(git_core::ASKPASS_ENV).is_some();
    if let Some(prompt) = askpass::prompt_from_args(&args, marker) {
        return askpass::main(prompt);
    }

    // Crash reports for the main process only (ADR-029).
    #[cfg(debug_assertions)]
    crash::debug_panic_trigger();
    if let Some(dir) = crash::default_dir() {
        crash::install(dir);
    }

    // `gitloom [PATH]...` opens each path in its own tab.
    let paths: Vec<PathBuf> = std::env::args_os()
        .skip(1)
        .map(|arg| {
            let path = PathBuf::from(arg);
            std::path::absolute(&path).unwrap_or(path)
        })
        .collect();

    let session_path = session::default_path();

    iced::application(
        move || App::new(paths.clone(), session_path.clone()),
        App::update,
        App::view,
    )
    .title(App::title)
    .subscription(App::subscription)
    .theme(App::theme)
    .scale_factor(App::scale_factor)
    .settings(iced::Settings {
        default_text_size: 13.into(),
        ..iced::Settings::default()
    })
    .window(app_icon::window_settings(iced::Size::new(1400.0, 860.0)))
    // The session is saved before the window closes.
    .exit_on_close_request(false)
    .run()
}
