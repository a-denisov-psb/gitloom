# Contributing

## Setup

- Rust via [rustup](https://rustup.rs). `rust-toolchain.toml` selects stable with rustfmt and clippy. MSRV: 1.88.
- `git` on PATH (needed by tests).
- Linux: X11/Wayland development libraries and `libxkbcommon`.

## Quality gate

Run before every commit. CI runs the same steps.

```sh
./scripts/check.sh        # Linux, macOS
scripts\check.ps1         # Windows
```

## Branching (git flow)

| Branch | From | Into | Purpose |
|---|---|---|---|
| `master` | – | – | Releases only, tagged `vX.Y.Z` |
| `develop` | `master` | – | Integration, always green |
| `feature/<task-id>-<slug>` | `develop` | `develop` | One task, e.g. `feature/M1-03-lane-layout` |
| `release/X.Y.Z` | `develop` | `master` + `develop` | Version bump, changelog, final fixes |
| `hotfix/X.Y.Z` | `master` | `master` + `develop` | Urgent fixes on a release |

- Merge with `--no-ff` so every feature stays visible as a unit in the graph.
- Delete feature branches after merging.
- Never rewrite history on `master` or `develop`.

### Feature

```sh
git switch develop
git switch -c feature/M1-03-lane-layout
# work, commit
./scripts/check.sh
git switch develop
git merge --no-ff feature/M1-03-lane-layout
git branch -d feature/M1-03-lane-layout
```

### Release

```sh
git switch -c release/0.2.0 develop
# bump version in Cargo.toml, finalize CHANGELOG.md
git commit -am "chore(release): 0.2.0"
git switch master
git merge --no-ff release/0.2.0
git tag -a v0.2.0 -m "v0.2.0"
git switch develop
git merge --no-ff release/0.2.0
git branch -d release/0.2.0
```

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org) with the crate as scope:

```text
feat(git-graph): assign lanes to merge parents
fix(app): keep selection after refresh
docs: describe session file format
chore(release): 0.2.0
```

Types: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `chore`, `ci`.

## Code conventions

- English everywhere (code, comments, docs).
- No `unwrap`/`expect`/`panic!` outside tests.
- UI follows GitKraken (`docs/UX-PARITY.md`); deviations are documented there.
- See `CLAUDE.md` for the full rule set.
