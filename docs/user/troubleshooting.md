# Troubleshooting

## Git not found or too old

Gitloom needs `git` 2.34 or newer on the `PATH`. Without it, a notice explains the reason and all write actions are disabled (browsing still works). Install Git (on Windows: Git for Windows), make sure `git --version` works in a new terminal, then restart Gitloom.

## Credentials

Gitloom uses Git's own credential handling:

- **HTTPS:** a configured credential helper (Git Credential Manager on Windows and macOS, `libsecret` or `store` on Linux) answers first. Otherwise Gitloom asks for user name and password or token in a small window. Many hosts (GitHub, GitLab) need a personal access token instead of the password.
- **SSH:** ssh-agent answers first. A passphrase-protected key without an agent opens Gitloom's prompt; unknown host keys ask Yes/No.
- The prompt window is Gitloom itself started by Git as `GIT_ASKPASS` / `SSH_ASKPASS`; Esc or Cancel aborts the operation.

If authentication fails, the error dialog shows Git's output with a hint. Check the remote URL (right click the remote → Edit URL), your token's permissions and `ssh -T git@<host>`.

## An operation fails because of a lock

Another Git process (an IDE, a terminal, a crashed tool) holds `index.lock` or a ref lock. Gitloom never deletes locks; finish or close the other program and choose **Retry**. Remove the lock file yourself only if you are sure no Git process is running.

## External tools

- **Diff and merge tools** come from Git (`git difftool --tool-help`). If none is configured, choose one in Preferences → External tools. Gitloom waits for the tool and refreshes when it closes. Only graphical tools work; terminal tools such as `vimdiff` get no window.
- **Terminal (Alt+T):** if no terminal is found or the wrong one opens, set a command in Preferences → External tools, e.g. `wt -d {path}` (Windows Terminal), `open -a iTerm {path}` (macOS) or `konsole --workdir {path}` (Linux).

## The folder dialog does not open (Linux)

The native dialog needs a running XDG desktop portal. Without it (minimal window managers, WSL), type the path into the field on the start screen.

## Gitloom closed unexpectedly

On the next start the status bar says "Gitloom closed unexpectedly" with **Open report folder** and **Copy report**. Please open an [issue](https://github.com/a-denisov-psb/gitloom/issues) with the report (check the message line for paths you do not want to share). Reports are only stored locally; the newest 20 are kept.

## Graphics problems

If the window stays black or rendering fails (old GPU drivers, virtual machines, remote desktops), start Gitloom with the software renderer: set the environment variable `ICED_BACKEND=tiny-skia`.

## Reset settings

Close Gitloom and delete `settings.json` (preferences) or `session.json` (tabs, recent repositories) from the config folder listed in [Settings](settings.md#files-on-disk).
