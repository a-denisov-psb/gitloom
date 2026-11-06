//! Opening the system terminal and file manager (M5-13). The commands are
//! built by pure functions (platform and executable lookup are passed in,
//! so the rules are testable everywhere); [`spawn`] starts them detached.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

/// What the system offers: executables on `PATH` and environment variables.
pub trait Lookup {
    fn has_program(&self, name: &str) -> bool;
    fn env(&self, key: &str) -> Option<String>;
}

/// The real system.
pub struct SystemLookup;

impl Lookup for SystemLookup {
    fn has_program(&self, name: &str) -> bool {
        let Some(paths) = std::env::var_os("PATH") else {
            return false;
        };
        let extensions: &[&str] = if cfg!(windows) {
            &["", ".exe", ".cmd", ".bat"]
        } else {
            &[""]
        };
        std::env::split_paths(&paths).any(|dir| {
            extensions
                .iter()
                .any(|ext| dir.join(format!("{name}{ext}")).is_file())
        })
    }

    fn env(&self, key: &str) -> Option<String> {
        std::env::var(key).ok().filter(|v| !v.trim().is_empty())
    }
}

/// A program to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub program: String,
    pub args: Vec<String>,
    /// Working directory.
    pub dir: PathBuf,
    /// Windows: the program needs its own console window.
    pub new_console: bool,
}

impl Launch {
    fn new(program: &str, args: Vec<String>, dir: &Path) -> Self {
        Self {
            program: program.to_owned(),
            args,
            dir: dir.to_path_buf(),
            new_console: false,
        }
    }
}

/// Terminal in `dir`: the command template from the settings (with
/// `{path}`) if set, else the first terminal found for the platform.
pub fn terminal(
    platform: Platform,
    lookup: &dyn Lookup,
    template: &str,
    dir: &Path,
) -> Result<Launch, String> {
    let path = dir.to_string_lossy().into_owned();
    if !template.trim().is_empty() {
        let mut words = split_words(template)?.into_iter();
        let program = words.next().ok_or("The terminal command is empty")?;
        let args = words.map(|w| w.replace("{path}", &path)).collect();
        return Ok(Launch::new(&program.replace("{path}", &path), args, dir));
    }
    let candidates: Vec<(String, Vec<String>, bool)> = match platform {
        Platform::Windows => vec![
            ("wt.exe".into(), vec!["-d".into(), path.clone()], false),
            ("pwsh".into(), vec!["-NoExit".into()], true),
            ("powershell".into(), vec!["-NoExit".into()], true),
            ("cmd".into(), vec!["/K".into()], true),
        ],
        Platform::MacOs => vec![(
            "open".into(),
            vec!["-a".into(), "Terminal".into(), path.clone()],
            false,
        )],
        Platform::Linux => {
            let mut list = Vec::new();
            if let Some(terminal) = lookup.env("TERMINAL") {
                list.push((terminal, Vec::new(), false));
            }
            list.extend([
                ("x-terminal-emulator".into(), Vec::new(), false),
                (
                    "gnome-terminal".into(),
                    vec![format!("--working-directory={path}")],
                    false,
                ),
                (
                    "konsole".into(),
                    vec!["--workdir".into(), path.clone()],
                    false,
                ),
                (
                    "xfce4-terminal".into(),
                    vec![format!("--working-directory={path}")],
                    false,
                ),
                (
                    "alacritty".into(),
                    vec!["--working-directory".into(), path.clone()],
                    false,
                ),
                (
                    "kitty".into(),
                    vec!["--directory".into(), path.clone()],
                    false,
                ),
                (
                    "wezterm".into(),
                    vec!["start".into(), "--cwd".into(), path.clone()],
                    false,
                ),
                ("xterm".into(), Vec::new(), false),
            ]);
            list
        }
    };
    candidates
        .into_iter()
        .find(|(program, _, _)| lookup.has_program(program))
        .map(|(program, args, new_console)| Launch {
            new_console,
            ..Launch::new(&program, args, dir)
        })
        .ok_or_else(|| {
            "No terminal found. Set a terminal command in Preferences → External tools.".to_owned()
        })
}

/// File manager showing `dir`.
pub fn file_manager(platform: Platform, dir: &Path) -> Launch {
    let program = match platform {
        Platform::Windows => "explorer",
        Platform::MacOs => "open",
        Platform::Linux => "xdg-open",
    };
    Launch::new(program, vec![dir.to_string_lossy().into_owned()], dir)
}

/// Default handler for `target`: a URL opens in the browser, a file in its
/// default application.
pub fn open_target(platform: Platform, target: &str) -> Launch {
    let program = match platform {
        Platform::Windows => "explorer",
        Platform::MacOs => "open",
        Platform::Linux => "xdg-open",
    };
    Launch::new(program, vec![target.to_owned()], &std::env::temp_dir())
}

/// File manager with `file` selected (where the platform can select).
pub fn reveal_file(platform: Platform, file: &Path) -> Launch {
    let dir = file.parent().unwrap_or(file);
    match platform {
        Platform::Windows => {
            Launch::new("explorer", vec![format!("/select,{}", file.display())], dir)
        }
        Platform::MacOs => Launch::new(
            "open",
            vec!["-R".into(), file.to_string_lossy().into_owned()],
            dir,
        ),
        Platform::Linux => file_manager(platform, dir),
    }
}

/// Splits a command template into words: whitespace separates, single and
/// double quotes group (and are removed), a backslash escapes the next
/// character outside single quotes.
pub fn split_words(template: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"') | None, '\\') => {
                if let Some(next) = chars.next() {
                    current.push(next);
                    in_word = true;
                }
            }
            (Some(_), c) => current.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                in_word = true;
            }
            (None, c) if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            (None, c) => {
                current.push(c);
                in_word = true;
            }
        }
    }
    if quote.is_some() {
        return Err("The terminal command has an unclosed quote".into());
    }
    if in_word {
        words.push(current);
    }
    Ok(words)
}

/// Starts `launch` detached (no stdin/stdout); returns once it runs.
pub fn spawn(launch: &Launch) -> Result<(), String> {
    let mut command = std::process::Command::new(&launch.program);
    command
        .args(&launch.args)
        .current_dir(&launch.dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        if launch.new_console {
            command.creation_flags(CREATE_NEW_CONSOLE);
        }
    }
    command
        .spawn()
        .map(drop)
        .map_err(|e| format!("Could not start {}: {e}", launch.program))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_open_with_the_platform_handler() {
        let url = "https://example.com/x";
        assert_eq!(open_target(Platform::Windows, url).program, "explorer");
        assert_eq!(open_target(Platform::MacOs, url).program, "open");
        let linux = open_target(Platform::Linux, url);
        assert_eq!(linux.program, "xdg-open");
        assert_eq!(linux.args, vec![url.to_owned()]);
    }

    struct Fake {
        programs: Vec<&'static str>,
        terminal: Option<&'static str>,
    }

    impl Lookup for Fake {
        fn has_program(&self, name: &str) -> bool {
            self.programs.contains(&name)
        }

        fn env(&self, key: &str) -> Option<String> {
            (key == "TERMINAL").then(|| self.terminal.map(str::to_owned))?
        }
    }

    fn fake(programs: &[&'static str], terminal: Option<&'static str>) -> Fake {
        Fake {
            programs: programs.to_vec(),
            terminal,
        }
    }

    fn command(launch: &Launch) -> Vec<String> {
        std::iter::once(launch.program.clone())
            .chain(launch.args.iter().cloned())
            .collect()
    }

    #[test]
    fn linux_detection_order() {
        let dir = Path::new("/home/me/my repo");
        let pick = |programs: &[&'static str], terminal| {
            terminal_command(Platform::Linux, &fake(programs, terminal), dir)
        };
        // $TERMINAL first (when it exists).
        assert_eq!(pick(&["foot", "xterm"], Some("foot")), ["foot"]);
        assert_eq!(
            pick(&["xterm"], Some("missing")),
            ["xterm"],
            "a $TERMINAL that is not installed is skipped"
        );
        assert_eq!(
            pick(&["x-terminal-emulator", "konsole"], None),
            ["x-terminal-emulator"]
        );
        assert_eq!(
            pick(&["konsole", "gnome-terminal"], None),
            ["gnome-terminal", "--working-directory=/home/me/my repo"]
        );
        assert_eq!(
            pick(&["kitty", "xterm"], None),
            ["kitty", "--directory", "/home/me/my repo"]
        );
        assert_eq!(
            pick(&["wezterm"], None),
            ["wezterm", "start", "--cwd", "/home/me/my repo"]
        );
        assert!(terminal(Platform::Linux, &fake(&[], None), "", dir)
            .unwrap_err()
            .contains("Preferences"));
    }

    fn terminal_command(platform: Platform, lookup: &Fake, dir: &Path) -> Vec<String> {
        let launch = terminal(platform, lookup, "", dir).unwrap();
        assert_eq!(launch.dir, dir, "the terminal starts in the repository");
        command(&launch)
    }

    #[test]
    fn windows_and_macos_detection_order() {
        let dir = Path::new(r"C:\repos\my repo");
        let windows = |programs: &[&'static str]| {
            terminal(Platform::Windows, &fake(programs, None), "", dir).unwrap()
        };
        assert_eq!(
            command(&windows(&["wt.exe", "pwsh", "cmd"])),
            ["wt.exe", "-d", r"C:\repos\my repo"]
        );
        let pwsh = windows(&["pwsh", "powershell", "cmd"]);
        assert_eq!(command(&pwsh), ["pwsh", "-NoExit"]);
        assert!(pwsh.new_console);
        assert_eq!(
            command(&windows(&["powershell", "cmd"])),
            ["powershell", "-NoExit"]
        );
        assert_eq!(command(&windows(&["cmd"])), ["cmd", "/K"]);

        let mac = terminal(Platform::MacOs, &fake(&["open"], None), "", Path::new("/r")).unwrap();
        assert_eq!(command(&mac), ["open", "-a", "Terminal", "/r"]);
    }

    #[test]
    fn template_expansion_with_spaces_and_quotes() {
        let dir = Path::new("/home/me/my repo");
        let custom = |template: &str| {
            command(&terminal(Platform::Linux, &fake(&[], None), template, dir).unwrap())
        };
        assert_eq!(
            custom("alacritty --working-directory {path}"),
            ["alacritty", "--working-directory", "/home/me/my repo"]
        );
        assert_eq!(
            custom(r#"wezterm start --cwd "{path}" -- bash -lc 'echo hi there'"#),
            [
                "wezterm",
                "start",
                "--cwd",
                "/home/me/my repo",
                "--",
                "bash",
                "-lc",
                "echo hi there"
            ]
        );
        assert_eq!(
            custom(r#""C:\Program Files\Term\term.exe" --dir={path}"#),
            [r"C:Program FilesTermterm.exe", "--dir=/home/me/my repo"],
            "backslashes escape inside double quotes"
        );
        assert_eq!(
            custom(r#"'C:\Program Files\Term\term.exe'"#),
            [r"C:\Program Files\Term\term.exe"],
            "single quotes keep backslashes"
        );
        assert!(terminal(Platform::Linux, &fake(&[], None), "term 'open", dir).is_err());
        assert_eq!(split_words("  a   b  ").unwrap(), ["a", "b"]);
        assert_eq!(split_words(r#"a "" b"#).unwrap(), ["a", "", "b"]);
    }

    #[test]
    fn file_managers() {
        let dir = Path::new("/r");
        assert_eq!(
            command(&file_manager(Platform::Linux, dir)),
            ["xdg-open", "/r"]
        );
        assert_eq!(command(&file_manager(Platform::MacOs, dir)), ["open", "/r"]);
        assert_eq!(
            command(&file_manager(Platform::Windows, dir)),
            ["explorer", "/r"]
        );
        let file = Path::new("/r/src/a.rs");
        assert_eq!(
            command(&reveal_file(Platform::MacOs, file)),
            ["open", "-R", "/r/src/a.rs"]
        );
        assert_eq!(
            command(&reveal_file(Platform::Linux, file)),
            ["xdg-open", "/r/src"]
        );
        assert_eq!(
            command(&reveal_file(Platform::Windows, file)),
            ["explorer", "/select,/r/src/a.rs"]
        );
    }
}
