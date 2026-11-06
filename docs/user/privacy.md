# Privacy

Gitloom has no telemetry, no accounts and no analytics. It contacts the network only in these cases:

| What | When | Who is contacted | What is sent |
|---|---|---|---|
| Git fetch, pull, push | When you run them, and auto-fetch every 5 minutes (Preferences → General, 0 = off) | The remotes you configured | What Git sends; Gitloom only starts `git` |
| Missing file contents of partial clones | When a diff needs a blob that was never downloaded | Your remote, through `git` | What Git sends |
| Avatars | Only when turned on (Preferences → Appearance, off by default) | gravatar.com | The SHA-256 hash of each visible author's email address; results are cached for 7 days |
| Update check | Only when turned on (Preferences → General, off by default); at most once a day | api.github.com | A request for the release list of the Gitloom repository |

With avatars and the update check off, Gitloom itself makes no network requests; no HTTP client is created.

**Credentials** are never stored by Gitloom. Git's credential helpers and ssh-agent answer first; Gitloom's prompt window passes what you type directly to Git.

**Crash reports** are written to your data folder (see [Settings](settings.md#files-on-disk)) and never uploaded. They contain the version, commit, operating system, thread name, the panic message and a backtrace. The panic message is written as is, so in rare cases it may contain a file path; check it before you attach a report to an issue.

Opening links (release notes, license, repository) starts your browser; Gitloom does not load web content itself.
