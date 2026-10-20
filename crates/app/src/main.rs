mod app;
mod format;
mod theme;
mod ui;

use app::App;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(theme::app_theme())
        .settings(iced::Settings {
            default_text_size: 13.into(),
            ..iced::Settings::default()
        })
        .window_size((1400.0, 860.0))
        .run()
}
