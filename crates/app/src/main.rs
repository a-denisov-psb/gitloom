mod app;
mod askpass;
mod fetch;
mod format;
mod menus;
mod nav;
mod overlay;
mod panes;
mod repo_view;
mod selection;
mod session;
mod shortcuts;
mod snapshot;
mod theme;
mod ui;
mod undo;

use std::path::PathBuf;

use app::App;

fn main() -> iced::Result {
    // Started by Git or SSH to ask for credentials (ADR-019).
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let marker = std::env::var_os(git_core::ASKPASS_ENV).is_some();
    if let Some(prompt) = askpass::prompt_from_args(&args, marker) {
        return askpass::main(prompt);
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
    .theme(theme::app_theme())
    .settings(iced::Settings {
        default_text_size: 13.into(),
        ..iced::Settings::default()
    })
    .window_size((1400.0, 860.0))
    // The session is saved before the window closes.
    .exit_on_close_request(false)
    .run()
}
