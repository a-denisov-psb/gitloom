# Security policy

## Supported versions

Security fixes are made for the latest release only. Please update before reporting.

## Reporting a vulnerability

Please do **not** open a public issue. Use GitHub's private vulnerability reporting instead: the repository's **Security** tab → **Report a vulnerability**. Only the maintainers see the report.

Please include the version (Help → About, or the command palette entry "About Gitloom"), your operating system, what an attacker can do and how to reproduce it. You can expect a first answer within a week.

## Scope

Gitloom runs the `git` executable and your configured tools (credential helpers, SSH, external diff and merge tools, terminals). Problems in those programs belong to their projects. In scope are, for example: Gitloom running commands or tools the user did not configure, leaking credentials typed into its prompt window, writing outside the repository, or network requests while the update check and avatars are off.
