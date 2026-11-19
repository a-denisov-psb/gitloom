//! Askpass mode (ADR-019): Git and SSH start Gitloom with a prompt when no
//! credential helper or ssh-agent can answer. A small window asks the user;
//! the answer goes to stdout, cancelling exits with code 1.

use std::io::Write as _;
use std::sync::{Arc, Mutex};

use iced::widget::{button, column, container, row, space, text, text_input};
use iced::{keyboard, Element, Fill, Task};

use crate::theme;

/// How a prompt is answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    /// Password, passphrase or PIN: masked input.
    Secret,
    /// SSH host key confirmation: Yes / No.
    YesNo,
    /// Anything else, e.g. a username.
    Text,
}

pub fn kind(prompt: &str) -> PromptKind {
    let lower = prompt.to_lowercase();
    if lower.contains("(yes/no") {
        PromptKind::YesNo
    } else if ["password", "passphrase", "pin", "token"]
        .iter()
        .any(|word| lower.contains(word))
    {
        PromptKind::Secret
    } else {
        PromptKind::Text
    }
}

/// The prompt if Gitloom was started in askpass mode: `--askpass <prompt>`,
/// or by Git/SSH with only the prompt while [`git_core::ASKPASS_ENV`] is set.
pub fn prompt_from_args(args: &[String], marker_set: bool) -> Option<String> {
    match args {
        [flag, rest @ ..] if flag == "--askpass" => Some(rest.join(" ")),
        [prompt] if marker_set => Some(prompt.clone()),
        [] if marker_set => Some(String::new()),
        _ => None,
    }
}

/// The theme of the prompt window (ADR-058): the theme chosen in the
/// settings (read only). With "System", or when the settings file or the
/// id is unusable, `None`: iced then follows the operating system's
/// preference, and [`theme_for`] picks the mapped theme once the mode is
/// known.
fn saved_appearance() -> Option<crate::settings::Appearance> {
    let path = crate::session::default_path()?.with_file_name(crate::settings::FILE_NAME);
    std::fs::metadata(&path).ok()?;
    Some(crate::settings::load(&path, None).settings.appearance)
}

/// Theme for the saved appearance and the operating system's mode.
fn theme_for(
    appearance: Option<&crate::settings::Appearance>,
    mode: iced::theme::Mode,
) -> Option<iced::Theme> {
    use crate::settings::ThemeChoice;
    let appearance = appearance?;
    let id = match &appearance.theme {
        ThemeChoice::Theme(id) => id,
        ThemeChoice::System => match mode {
            iced::theme::Mode::Light => &appearance.system_light,
            iced::theme::Mode::Dark => &appearance.system_dark,
            // Not known yet: iced follows the operating system.
            iced::theme::Mode::None => return None,
        },
    };
    crate::themes::get(id).map(|def| theme::iced_theme(&def))
}

/// Shows the prompt window, writes the answer to stdout and exits.
pub fn main(prompt: String) -> iced::Result {
    let answer: Arc<Mutex<Option<String>>> = Arc::default();
    let slot = Arc::clone(&answer);
    let height = window_height(&prompt);
    iced::application(
        move || Askpass::new(prompt.clone(), Arc::clone(&slot)),
        Askpass::update,
        Askpass::view,
    )
    .title(|_: &Askpass| format!("{} – Authentication", git_core::identity::DISPLAY_NAME))
    // Also when the focused input captured the key.
    .subscription(|_: &Askpass| iced::event::listen_with(|event, _status, _window| escape(event)))
    .theme(|state: &Askpass| theme_for(state.appearance.as_ref(), state.system_mode))
    .settings(iced::Settings {
        default_text_size: 13.into(),
        ..iced::Settings::default()
    })
    .window(crate::app_icon::window_settings(iced::Size::new(
        480.0, height,
    )))
    .resizable(false)
    .centered()
    .level(iced::window::Level::AlwaysOnTop)
    .run()?;

    let answer = answer.lock().ok().and_then(|mut a| a.take());
    match answer {
        Some(answer) => {
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{answer}");
            let _ = out.flush();
            std::process::exit(0)
        }
        None => std::process::exit(1),
    }
}

/// Window height for `prompt`: SSH host key prompts span several lines
/// (host, fingerprint, question); about 64 characters fit on a line.
fn window_height(prompt: &str) -> f32 {
    let lines: usize = prompt
        .trim()
        .lines()
        .map(|line| line.chars().count().div_ceil(64).max(1))
        .sum();
    (110.0 + 20.0 * lines as f32).clamp(160.0, 420.0)
}

fn escape(event: iced::Event) -> Option<Message> {
    match event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            ..
        }) => Some(Message::Cancel),
        _ => None,
    }
}

const INPUT: &str = "askpass-input";

#[derive(Debug, Clone)]
pub enum Message {
    Input(String),
    Submit,
    Answer(&'static str),
    Cancel,
    SystemTheme(iced::theme::Mode),
}

struct Askpass {
    prompt: String,
    kind: PromptKind,
    input: String,
    answer: Arc<Mutex<Option<String>>>,
    /// Appearance from `settings.json`, read once at start.
    appearance: Option<crate::settings::Appearance>,
    system_mode: iced::theme::Mode,
}

impl Askpass {
    fn new(prompt: String, answer: Arc<Mutex<Option<String>>>) -> (Self, Task<Message>) {
        let state = Self {
            kind: kind(&prompt),
            prompt,
            input: String::new(),
            answer,
            appearance: saved_appearance(),
            system_mode: iced::theme::Mode::None,
        };
        let focus = iced::widget::operation::focus(INPUT);
        let mode = iced::system::theme().map(Message::SystemTheme);
        (state, Task::batch([focus, mode]))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let answer = match message {
            Message::Input(input) => {
                self.input = input;
                return Task::none();
            }
            Message::Submit => Some(self.input.clone()),
            Message::Answer(answer) => Some(answer.to_owned()),
            Message::Cancel => None,
            Message::SystemTheme(mode) => {
                self.system_mode = mode;
                return Task::none();
            }
        };
        if let Ok(mut slot) = self.answer.lock() {
            *slot = answer;
        }
        iced::exit()
    }

    fn view(&self) -> Element<'_, Message> {
        let prompt = text(self.prompt.trim().to_owned());
        let buttons = match self.kind {
            PromptKind::YesNo => row![
                space::horizontal(),
                button(text("No"))
                    .on_press(Message::Answer("no"))
                    .style(button::secondary),
                button(text("Yes")).on_press(Message::Answer("yes")),
            ],
            PromptKind::Secret | PromptKind::Text => row![
                space::horizontal(),
                button(text("Cancel"))
                    .on_press(Message::Cancel)
                    .style(button::secondary),
                button(text("OK")).on_press(Message::Submit),
            ],
        }
        .spacing(8);
        let mut content = column![prompt].spacing(12);
        if self.kind != PromptKind::YesNo {
            content = content.push(
                text_input("", &self.input)
                    .id(INPUT)
                    .secure(self.kind == PromptKind::Secret)
                    .on_input(Message::Input)
                    .on_submit(Message::Submit)
                    .padding(6)
                    .style(theme::input),
            );
        }
        container(content.push(buttons))
            .padding(16)
            .width(Fill)
            .height(Fill)
            .style(theme::root)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_kinds() {
        assert_eq!(
            kind("Username for 'https://github.com': "),
            PromptKind::Text
        );
        assert_eq!(
            kind("Password for 'https://me@github.com': "),
            PromptKind::Secret
        );
        assert_eq!(
            kind("Enter passphrase for key '/home/me/.ssh/id_ed25519': "),
            PromptKind::Secret
        );
        assert_eq!(
            kind("Are you sure you want to continue connecting (yes/no/[fingerprint])? "),
            PromptKind::YesNo
        );
    }

    #[test]
    fn escape_cancels_and_long_prompts_get_a_taller_window() {
        let esc = iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            modified_key: keyboard::Key::Named(keyboard::key::Named::Escape),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Escape),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        });
        assert!(matches!(escape(esc), Some(Message::Cancel)));

        let host_key = "The authenticity of host 'github.com (140.82.121.4)' can't be established.\n\
                        ED25519 key fingerprint is SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU.\n\
                        This key is not known by any other names.\n\
                        Are you sure you want to continue connecting (yes/no/[fingerprint])? ";
        assert!(window_height(host_key) > window_height("Password: "));
        assert_eq!(window_height("Password: "), 160.0);
    }

    #[test]
    fn askpass_mode_from_arguments() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            prompt_from_args(&args(&["--askpass", "Password:"]), false),
            Some("Password:".into())
        );
        assert_eq!(
            prompt_from_args(&args(&["Password:"]), true),
            Some("Password:".into())
        );
        // Without the marker a single argument is a repository path.
        assert_eq!(prompt_from_args(&args(&["/repo"]), false), None);
        assert_eq!(prompt_from_args(&args(&["/a", "/b"]), true), None);
    }

    /// ADR-058: the prompt uses the saved theme; System uses the mapped
    /// theme once the mode is known; without a usable file or id iced
    /// follows the operating system (`None`).
    #[test]
    fn prompt_theme_follows_the_settings() {
        use crate::settings::{Appearance, ThemeChoice};
        let name = |theme: Option<iced::Theme>| theme.map(|t| t.to_string());
        let mut appearance = Appearance {
            theme: ThemeChoice::theme("light"),
            ..Appearance::default()
        };
        for mode in [iced::theme::Mode::None, iced::theme::Mode::Dark] {
            assert_eq!(
                name(theme_for(Some(&appearance), mode)).as_deref(),
                Some("light")
            );
        }
        appearance.theme = ThemeChoice::System;
        appearance.system_light = "light".into();
        appearance.system_dark = "dark".into();
        let light = theme_for(Some(&appearance), iced::theme::Mode::Light);
        assert_eq!(name(light).as_deref(), Some("light"));
        let dark = theme_for(Some(&appearance), iced::theme::Mode::Dark);
        assert_eq!(name(dark).as_deref(), Some("dark"));
        assert_eq!(theme_for(Some(&appearance), iced::theme::Mode::None), None);
        appearance.theme = ThemeChoice::theme("no-such-theme");
        assert_eq!(theme_for(Some(&appearance), iced::theme::Mode::Dark), None);
        // No settings file.
        assert_eq!(theme_for(None, iced::theme::Mode::Light), None);
    }
}
