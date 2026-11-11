# Contributing

Bug reports and ideas are welcome as [issues](https://github.com/a-denisov-psb/gitloom/issues); security problems go through a private report (see [SECURITY.md](SECURITY.md)). Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md). Gitloom follows GitKraken's layout and shortcuts, so a feature request should say how GitKraken does it; accounts, cloud features, integrations and AI features are out of scope.

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

## Building

Optimized builds use [`just`](https://github.com/casey/just) as task runner and the Cargo profile `dist` (full LTO, one codegen unit; ADR-052). Install `just` once:

```sh
cargo install --locked just     # Linux, macOS, WSL
winget install Casey.Just       # Windows
```

| Recipe | What it does |
|---|---|
| `just` | Lists the recipes |
| `just check` | Quality gate (`scripts/check.sh` / `check.ps1`) |
| `just build <target>` | Builds one binary with `--profile dist` into `dist/<bin>-<version>-<target>[.exe]` and prints path and size |
| `just portable` | Same as `just build win-x64` |
| `just targets` | Target aliases and whether this machine can build them |
| `just package` | Release packages of this platform into `dist/` (`scripts/package.sh` / `package.ps1`) |
| `just ci-packages [ref]` | Runs the release workflow as a dry run on `ref` (default `develop`), waits and downloads all packages into `dist/ci-<short-sha>/`; needs a logged-in `gh` |
| `just clean` | `cargo clean` and removes `dist/` (the next build takes longer) |

Targets of `just build`:

| Target | On Linux/WSL | On Windows |
|---|---|---|
| `win-x64` | `x86_64-pc-windows-gnu`, needs mingw-w64 | `x86_64-pc-windows-msvc` |
| `win-x86` | `i686-pc-windows-gnu`, needs mingw-w64 | `i686-pc-windows-msvc` |
| `win-arm64` | not possible, use `just ci-packages` | `aarch64-pc-windows-msvc`, needs the MSVC ARM64 build tools |
| `linux-x64` | `x86_64-unknown-linux-gnu` | use WSL |
| `linux-arm64` | `aarch64-unknown-linux-gnu`; native on ARM64, else `cargo zigbuild` | use WSL |
| `mac` | not possible, use `just ci-packages` | not possible, use `just ci-packages` |

Prerequisites, named by `just build` when missing:

- the Rust target: `rustup target add <triple>`
- Windows from Linux/WSL: `sudo apt install mingw-w64`
- Linux ARM64 from x64: `cargo install --locked cargo-zigbuild` and zig (`pip3 install ziglang` or [ziglang.org](https://ziglang.org/download/))

Release packages come from CI (ADR-016): tags on `master` build and attach them, the manual dry run (`just ci-packages`) only uploads them as workflow artifacts. Windows x86/ARM64 and Linux ARM64 are best effort.

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

## License of contributions

Gitloom is licensed under `GPL-3.0-or-later` (see [LICENSE](LICENSE)). By submitting a contribution you agree that it is licensed under the same terms. No CLA and no DCO sign-off are required. New source files need no license header; if you add one, use the SPDX form:

```rust
// SPDX-License-Identifier: GPL-3.0-or-later
```

New dependencies must have a license compatible with GPL-3.0; `cargo deny check` (part of `./scripts/check.sh` when installed, and of CI) enforces the allow list in `deny.toml`. After adding or updating a dependency, regenerate the third-party notices with `./scripts/third-party-licenses.sh`.
