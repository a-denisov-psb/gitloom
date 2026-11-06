## What and why

<!-- What does this change, and which issue or milestone task does it belong to? -->

## Checklist

- [ ] `./scripts/check.sh` passes (fmt, clippy, tests; cargo-deny and the third-party notices when installed)
- [ ] New logic has tests; a bug fix has a regression test
- [ ] UI changes follow GitKraken (`docs/UX-PARITY.md`); deviations are listed there
- [ ] `CHANGELOG.md` → `[Unreleased]` updated for user-visible changes
- [ ] New dependencies: license allowed by `deny.toml`, notices regenerated (`./scripts/third-party-licenses.sh`)

By submitting this pull request I agree that my contribution is licensed under GPL-3.0-or-later.
