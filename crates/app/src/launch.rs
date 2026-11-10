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
    /// Full path of `name` on `PATH`; on Windows with the extensions of
    /// `PATHEXT` (`code` → `code.cmd`, M8-07).
    fn resolve(&self, name: &str) -> Option<PathBuf>;
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

    fn resolve(&self, name: &str) -> Option<PathBuf> {
        let paths = std::env::var_os("PATH")?;
        let extensions = path_extensions(Platform::current(), self.env("PATHEXT").as_deref());
        std::env::split_paths(&paths).find_map(|dir| {
            extensions
                .iter()
                .map(|ext| dir.join(format!("{name}{ext}")))
                .find(|candidate| candidate.is_file())
        })
    }
}

/// Extensions tried after a program name: on Windows `""` and those of
/// `PATHEXT` (default `.COM;.EXE;.BAT;.CMD`), elsewhere just `""`.
pub fn path_extensions(platform: Platform, pathext: Option<&str>) -> Vec<String> {
    match platform {
        Platform::Windows => {
            let list = pathext.unwrap_or(".COM;.EXE;.BAT;.CMD");
            std::iter::once(String::new())
                .chain(
                    list.split(';')
                        .filter(|ext| !ext.trim().is_empty())
                        .map(|ext| ext.trim().to_lowercase()),
                )
                .collect()
        }
        Platform::MacOs | Platform::Linux => vec![String::new()],
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
    /// Windows: start without a console window (batch launchers such as
    /// `code.cmd`, M8-07).
    pub no_window: bool,
    /// Windows: appended to the command line verbatim, after `args`
    /// (Explorer parses `/select,"<path>"` itself, so Rust's quoting of the
    /// whole argument would break paths with spaces).
    pub raw_arg: Option<String>,
}

impl Launch {
    fn new(program: &str, args: Vec<String>, dir: &Path) -> Self {
        Self {
            program: program.to_owned(),
            args,
            dir: dir.to_path_buf(),
            new_console: false,
            no_window: false,
            raw_arg: None,
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
        let mut words = split_words_for(platform, template)?.into_iter();
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

/// Why no editor can be opened: none is set.
pub const NO_EDITOR: &str = "Set an external editor in Preferences → External tools";

/// External editor for `target` (M8-07): the command template from the
/// settings with the terminal's rules ([`split_words`], `{path}`); without
/// `{path}` the path is appended. On Windows a bare program name is
/// resolved through `PATHEXT` and batch launchers start without a console
/// window.
pub fn editor(
    platform: Platform,
    lookup: &dyn Lookup,
    template: &str,
    target: &Path,
) -> Result<Launch, String> {
    if template.trim().is_empty() {
        return Err(NO_EDITOR.to_owned());
    }
    let path = target.to_string_lossy().into_owned();
    let mut words = split_words_for(platform, template)
        .map_err(|e| e.replace("terminal command", "editor command"))?
        .into_iter();
    let program = words
        .next()
        .ok_or("The editor command is empty")?
        .replace("{path}", &path);
    let mut args: Vec<String> = words.map(|w| w.replace("{path}", &path)).collect();
    if !template.contains("{path}") {
        args.push(path);
    }
    let dir = if target.is_dir() {
        target.to_path_buf()
    } else {
        target.parent().unwrap_or(target).to_path_buf()
    };
    let mut launch = Launch::new(&program, args, &dir);
    if platform == Platform::Windows {
        let bare = !program.contains(['/', '\\']) && Path::new(&program).extension().is_none();
        if bare {
            if let Some(found) = lookup.resolve(&program) {
                launch.program = found.to_string_lossy().into_owned();
            }
        }
        let batch = Path::new(&launch.program)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));
        launch.no_window = batch;
    }
    Ok(launch)
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

/// Absolute path of the repository file `rel` (relative to `root`, with
/// `/` as Git writes it): native separators and, on Windows, without the
/// `\\?\` prefix of verbatim paths, which Explorer and the clipboard
/// consumers do not expect (M6-21).
pub fn repo_file_path(root: &Path, rel: &str) -> PathBuf {
    match root.to_str() {
        Some(root) => PathBuf::from(native_join(Platform::current(), root, rel)),
        // Not UTF-8 (Unix only): join without touching the root.
        None => {
            let mut path = root.to_path_buf();
            path.extend(rel_parts(rel));
            path
        }
    }
}

/// [`repo_file_path`] for `platform`, on strings.
pub fn native_join(platform: Platform, root: &str, rel: &str) -> String {
    let separator = match platform {
        Platform::Windows => '\\',
        Platform::MacOs | Platform::Linux => '/',
    };
    let mut path = match platform {
        Platform::Windows => {
            let root = if let Some(rest) = root.strip_prefix(r"\\?\UNC\") {
                format!(r"\\{rest}")
            } else {
                root.strip_prefix(r"\\?\").unwrap_or(root).to_owned()
            };
            root.replace('/', "\\")
        }
        Platform::MacOs | Platform::Linux => root.to_owned(),
    };
    while path.len() > 1 && path.ends_with(separator) && !path.ends_with(&format!(":{separator}")) {
        path.pop();
    }
    for part in rel_parts(rel) {
        if !path.ends_with(separator) {
            path.push(separator);
        }
        path.push_str(part);
    }
    path
}

fn rel_parts(rel: &str) -> impl Iterator<Item = &str> {
    rel.split('/')
        .filter(|part| !part.is_empty() && *part != ".")
}

/// File manager with `file` selected (where the platform can select). A
/// file that no longer exists (`exists` says no) shows its nearest existing
/// folder on Windows.
pub fn reveal_file(platform: Platform, file: &Path, exists: &dyn Fn(&Path) -> bool) -> Launch {
    // Start in a folder that exists: a deleted folder cannot be the
    // working directory.
    let folder = file
        .ancestors()
        .skip(1)
        .find(|dir| exists(dir))
        .unwrap_or_else(|| file.parent().unwrap_or(file));
    match platform {
        Platform::Windows if exists(file) => Launch {
            raw_arg: Some(format!("/select,\"{}\"", file.display())),
            ..Launch::new("explorer", Vec::new(), folder)
        },
        Platform::Windows => file_manager(platform, folder),
        Platform::MacOs => Launch::new(
            "open",
            vec!["-R".into(), file.to_string_lossy().into_owned()],
            folder,
        ),
        Platform::Linux => file_manager(platform, folder),
    }
}

/// Splits a command template into words: whitespace separates, single and
/// double quotes group (and are removed), a backslash escapes the next
/// character outside single quotes, except on Windows, where it is a path
/// separator, so `"C:\Program Files\Editor\ed.exe"` keeps its
/// backslashes (M8-07).
pub fn split_words_for(platform: Platform, template: &str) -> Result<Vec<String>, String> {
    match platform {
        Platform::Windows => split(template, false),
        Platform::MacOs | Platform::Linux => split(template, true),
    }
}

/// [`split_words_for`] with the Unix rules.
#[cfg(test)]
pub fn split_words(template: &str) -> Result<Vec<String>, String> {
    split(template, true)
}

fn split(template: &str, escapes: bool) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"') | None, '\\') if escapes => {
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
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        if launch.new_console {
            command.creation_flags(CREATE_NEW_CONSOLE);
        } else if launch.no_window {
            command.creation_flags(CREATE_NO_WINDOW);
        }
        if let Some(raw) = &launch.raw_arg {
            command.raw_arg(raw);
        }
    }
    #[cfg(not(windows))]
    if let Some(raw) = &launch.raw_arg {
        command.arg(raw);
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

        fn resolve(&self, name: &str) -> Option<PathBuf> {
            self.programs
                .iter()
                .find(|p| p.strip_suffix(".cmd").or(Some(p)) == Some(name) || **p == name)
                .map(|p| PathBuf::from(format!(r"C:\Tools\{p}")))
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
            .chain(launch.raw_arg.iter().cloned())
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
        let all = |_: &Path| true;
        assert_eq!(
            command(&reveal_file(Platform::MacOs, file, &all)),
            ["open", "-R", "/r/src/a.rs"]
        );
        assert_eq!(
            command(&reveal_file(Platform::Linux, file, &all)),
            ["xdg-open", "/r/src"]
        );
        assert_eq!(
            command(&reveal_file(Platform::Windows, file, &all)),
            ["explorer", "/select,\"/r/src/a.rs\""]
        );
    }

    #[test]
    fn repository_file_paths_use_native_separators() {
        let win = |root: &str, rel: &str| native_join(Platform::Windows, root, rel);
        let unix = |root: &str, rel: &str| native_join(Platform::Linux, root, rel);
        assert_eq!(
            win(r"C:\Users\me\git test", "sub dir/a b.txt"),
            r"C:\Users\me\git test\sub dir\a b.txt"
        );
        assert_eq!(
            unix("/home/me/git test", "sub dir/a b.txt"),
            "/home/me/git test/sub dir/a b.txt"
        );
        // Umlauts and characters that are special in shells or Explorer.
        for name in ["Übersicht ä.md", "#1 & more, too.txt"] {
            assert_eq!(
                win(r"C:\Repos\Grüße & Co", &format!("dir #2/{name}")),
                format!(r"C:\Repos\Grüße & Co\dir #2\{name}")
            );
            assert_eq!(
                unix("/r/Grüße & Co", &format!("dir #2/{name}")),
                format!("/r/Grüße & Co/dir #2/{name}")
            );
        }
        // Verbatim prefix, forward slashes and trailing separators.
        assert_eq!(win(r"\\?\C:\repo", "a/b.rs"), r"C:\repo\a\b.rs");
        assert_eq!(
            win(r"\\?\UNC\server\share\repo", "a.rs"),
            r"\\server\share\repo\a.rs"
        );
        assert_eq!(win("C:/work/repo/", "src/a.rs"), r"C:\work\repo\src\a.rs");
        assert_eq!(win(r"D:\", "a.rs"), r"D:\a.rs");
        assert_eq!(unix("/", "a.rs"), "/a.rs");
        assert_eq!(unix("/r/", "./a.rs"), "/r/a.rs");
        // The real helper on this platform.
        let root = std::env::temp_dir().join("git test");
        let path = repo_file_path(&root, "sub/a b.txt");
        assert_eq!(path, root.join("sub").join("a b.txt"));
    }

    #[test]
    fn windows_reveal_command_line() {
        let file = Path::new(r"C:\Users\me\git test\sub, dir\a & b #1.txt");
        let launch = reveal_file(Platform::Windows, file, &|_| true);
        assert_eq!(launch.program, "explorer");
        assert!(launch.args.is_empty());
        assert_eq!(
            launch.raw_arg.as_deref(),
            Some(r#"/select,"C:\Users\me\git test\sub, dir\a & b #1.txt""#)
        );

        // Deleted file (and folder): the nearest existing folder opens.
        let gone = Path::new("/r/gone/deeper/a.txt");
        let exists = |p: &Path| p == Path::new("/r") || p == Path::new("/");
        let launch = reveal_file(Platform::Windows, gone, &exists);
        assert_eq!(command(&launch), ["explorer", "/r"]);
        assert_eq!(launch.dir, Path::new("/r"));
        assert_eq!(launch.raw_arg, None);
    }

    #[test]
    fn editor_templates() {
        let lookup = fake(&[], None);
        let file = Path::new("/r/my repo/src/a b.rs");
        let launch = |template: &str| editor(Platform::Linux, &lookup, template, file);
        assert_eq!(
            command(&launch("code {path}").unwrap()),
            ["code", "/r/my repo/src/a b.rs"]
        );
        // Without {path} the path is the last argument.
        assert_eq!(
            command(&launch("subl -n").unwrap()),
            ["subl", "-n", "/r/my repo/src/a b.rs"]
        );
        // Quotes group words; {path} inside a word.
        assert_eq!(
            command(&launch(r#""/opt/My Editor/bin/edit" --goto={path} -w"#).unwrap()),
            [
                "/opt/My Editor/bin/edit",
                "--goto=/r/my repo/src/a b.rs",
                "-w"
            ]
        );
        assert_eq!(launch("  ").unwrap_err(), NO_EDITOR);
        assert!(launch("code 'open").unwrap_err().contains("editor command"));
        // The working directory is the file's folder.
        assert_eq!(launch("code").unwrap().dir, Path::new("/r/my repo/src"));
    }

    #[test]
    fn windows_editor_launchers_resolve_through_pathext() {
        let lookup = fake(&["code.cmd", "idea64.exe"], None);
        let file = Path::new(r"C:\repo\a.rs");
        let launch = editor(Platform::Windows, &lookup, "code {path}", file).unwrap();
        assert_eq!(launch.program, r"C:\Tools\code.cmd");
        assert!(launch.no_window, "batch launcher without a console window");
        // A path or an extension is used as given.
        let launch = editor(Platform::Windows, &lookup, r#""C:\Ed\ed.exe" {path}"#, file).unwrap();
        assert_eq!(launch.program, r"C:\Ed\ed.exe");
        assert!(!launch.no_window);
        // Not found: the name stays (the start then fails with a message).
        let launch = editor(Platform::Windows, &lookup, "zed", file).unwrap();
        assert_eq!(launch.program, "zed");
        assert_eq!(
            path_extensions(Platform::Windows, Some(".COM;.EXE;.BAT;.CMD")),
            ["", ".com", ".exe", ".bat", ".cmd"]
        );
        assert_eq!(path_extensions(Platform::Windows, None).len(), 5);
        assert_eq!(path_extensions(Platform::Linux, Some(".EXE")), [""]);
        // Elsewhere nothing is resolved.
        let launch = editor(Platform::Linux, &lookup, "code {path}", file).unwrap();
        assert_eq!(launch.program, "code");
        assert!(!launch.no_window);
    }

    #[test]
    fn windows_templates_keep_backslashes() {
        let words = split_words_for(
            Platform::Windows,
            r#""C:\Program Files\Ed\ed.exe" --wait C:\x\{path}"#,
        )
        .unwrap();
        assert_eq!(
            words,
            [r"C:\Program Files\Ed\ed.exe", "--wait", r"C:\x\{path}"]
        );
        // Elsewhere a backslash still escapes (M5-13 rule).
        assert_eq!(
            split_words_for(Platform::Linux, r#"a\ b "c\"d""#).unwrap(),
            ["a b", "c\"d"]
        );
    }
}
