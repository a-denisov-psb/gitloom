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

/// The prompt follows the theme chosen in the settings (System counts as
/// Dark here: the prompt has no time to ask the window system).
fn prefers_light() -> bool {
    crate::session::default_path()
        .map(|path| path.with_file_name(crate::settings::FILE_NAME))
        .is_some_and(|path| {
            crate::settings::load(&path, None).settings.appearance.theme
                == crate::settings::ThemeChoice::Light
        })
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
    .title("Gitloom – Authentication")
    // Also when the focused input captured the key.
    .subscription(|_: &Askpass| iced::event::listen_with(|event, _status, _window| escape(event)))
    .theme(theme::app_theme(prefers_light()))
    .settings(iced::Settings {
        default_text_size: 13.into(),
        ..iced::Settings::default()
    })
    .window_size((480.0, height))
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
}

struct Askpass {
    prompt: String,
    kind: PromptKind,
    input: String,
    answer: Arc<Mutex<Option<String>>>,
}

impl Askpass {
    fn new(prompt: String, answer: Arc<Mutex<Option<String>>>) -> (Self, Task<Message>) {
        let state = Self {
            kind: kind(&prompt),
            prompt,
            input: String::new(),
            answer,
        };
        let focus = iced::widget::operation::focus(INPUT);
        (state, focus)
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
                    .padding(6),
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
}
