# Performance

Measurements for M1 (task M1-15). Release builds, numbers are medians of 3 runs unless noted.

## Machine

| | |
|---|---|
| CPU | Intel Core Ultra 9 285H (16 threads) |
| RAM | 32 GB |
| OS | Windows with WSL2 (Linux 6.18), app shown through WSLg / XWayland |
| Rust | 1.99.1 (`stable` via rustup), `lto = "thin"` |
| Test repository | `rust-lang/rust`, blobless clone (`--filter=blob:none`), 349 495 commits, 176 refs, 63 812 files; 100 000 commits loaded |

## Results

| Metric | Target | Result | How |
|---|---|---|---|
| Topo order + layout, 100k synthetic commits (29 lanes) | < 100 ms | **14.8 ms** (topo 6.3 ms, layout 7.9 ms) | `cargo bench -p git-graph` |
| Open + log(100 000) + refs, rust-lang/rust | < 1.5 s (M1-04) | **0.76 s** | `GITLOOM_PERF_REPO=<path> cargo test -p git-core --release --test perf -- --ignored --nocapture` |
| Full snapshot (log, refs, status, topo, layout; 254 lanes max) | – | **0.96 s** | `GITLOOM_PERF_REPO=<path> cargo test -p gitloom --release snapshot_memory -- --ignored --nocapture` |
| Open large repo → first graph paint | < 2 s | **1.21 s** (process start to painted rows) | screenshot polling via X11 |
| Status on large clean repo | < 1 s | **65 ms** | perf test `status` |
| Memory with 100k commits loaded (private memory, ADR-012) | < 300 MB | **146 MB** private (`RssAnon`); reported separately: 208 MB memory-mapped pack files, 42 MB shared GPU buffers, 396 MB total RSS | `/proc/<pid>/status` after the graph is shown |
| Scrolling 100k rows | fluid | Only visible rows (+10 overscan) are built per frame; wheel and scrollbar dragging work without visible delay in WSLg | by hand |

## M2: working copy (task M2-16)

Same machine and repository. Release builds.

| Metric | Target | Result | How |
|---|---|---|---|
| Status refresh after one file change (status + snapshot update, history reused) | < 200 ms | **78 ms** median of 7 (75–83 ms) | `GITLOOM_PERF_REPO=<path> cargo test -p gitloom --release status_refresh_after_one_change -- --ignored --nocapture` (modifies and restores `README.md`) |
| Status on large clean repo | < 1 s | **87 ms** | perf test `status` |
| Watcher idle CPU, rust-lang/rust open | ≈ 0 % | **0.00 %** (0 ticks in 60 s); was 0.48 % before the fix below | `/proc/<pid>/stat` utime + stime over 60 s |
| Refresh caused by ignored paths | none | 20 000 files written into ignored `build/` and `target/`: 0.15 s CPU in total for event handling and classification, no refresh; `classify_changes` of 11 000 ignored paths: 15 ms, empty result | by hand; perf test `ignored_build_output_causes_no_refresh` |
| External change visible | < 1 s | WIP row shown 1 s after appending to `README.md` (screenshot) | by hand |
| Stage one hunk of a 1.8 MB file (340 hunks) | – | diff **16 ms**, build patch + `git apply --cached` **8 ms** | `cargo test -p git-core --release --test perf stage_hunk -- --ignored --nocapture` |
| Lane layout, 100k commits, without hints (M2-15) | no regression | 7.9–8.1 ms vs. 7.5–7.9 ms before (run-to-run variance) | `cargo bench -p git-graph` |

### Fixed in M2-16

- **Watcher feedback loop**: batches consisting only of access events were still classified; classifying opens `.git/info/exclude`, which produced the next access event, about three times per second (0.48 % CPU, one tokio worker). Access events are now dropped in the watcher callback.

## M3: branches and remotes (task M3-14)

| Metric | Result | How |
|---|---|---|
| Network runner overhead (fetch of 300 commits from a local remote) | **102.3 ms** vs. 99.6 ms plain `git fetch --quiet` (≈ 3 ms incl. ~400 progress updates per fetch) | `cargo test -p git-core --release --test perf network_runner -- --ignored --nocapture` |
| Idle CPU with auto-fetch on (5 min), repository with a GitHub remote | **0.02 %** over 120 s (was 0.30 % with a 30 s tick, see below) | `/proc/<pid>/stat` incl. children, release build, software rendering (llvmpipe) |
| Auto-fetch every minute against GitHub (SSH) | 0.29 % over 130 s with two fetches ≈ 0.19 s CPU per fetch including the `git`/`ssh` processes | same |

### Fixed in M3-14

- **Auto-fetch tick**: a subscription firing every 30 s made iced redraw the window each time; with software rendering (llvmpipe, no GPU in WSLg) that cost 0.3 % CPU at idle. Auto-fetch now arms one timer for the next due fetch (`fetch::next_check`, retry after 60 s when a tab was busy).

## M4: merge, rebase, conflicts (task M4-14)

Release builds, same machine. Interaction latencies are measured from the synthetic click to the first changed pixels (X11 screenshot polling); the same method gives **81 ms for a plain sidebar click**, the floor of this WSLg/llvmpipe setup.

| Metric | Result | How |
|---|---|---|
| Conflict model, 5 000-line file with 500 conflicts | load (`ls-files -u`, 3 blobs, `git merge-file`, parse) **6.6 ms**; take all theirs + output **0.18 ms** | `cargo test -p git-core --release --test perf conflict_of_a_5000 -- --ignored --nocapture` |
| Conflict view of that file in the app | opening and toggling a line or "Take all theirs": **81–82 ms**, i.e. no measurable cost above the floor | by hand, release build |
| Interactive rebase of 200 commits (19 squashes, 10 rewords, one reorder) | loading the commits **2.7 ms**; `rebase -i` with Gitloom as editor (29 message calls) **0.38 s** | `cargo test -p gitloom --release --test perf -- --ignored --nocapture` |
| Interactive rebase panel with 200 commits | opening and changing actions: **82–84 ms** (floor) | by hand, release build |

### Fixed in M4-14

- **Context menus in a scrolled graph** appeared off screen: inside a scrolled `scrollable`, iced passes the cursor in content coordinates. `context_area`/`drag_area` now add the translation iced hands to `overlay()` at the start of every update.
- **Scroll position reset when a menu opened**: the root switched between a bare widget and a `Stack` when an overlay appeared, so iced rebuilt the widget tree and dropped the graph's scroll state; the root is now always a `Stack`.

## M5: productivity parity (task M5-14)

Release builds, same machine. The generated repository comes from
`scripts/perf_repo_m5.py` (100 000 commits, 50 authors, 21 branches, `hot.txt` changed by
2 000 commits, `big.txt` with 10 000 lines changed by 200 commits); measured with
`GITLOOM_PERF_REPO=<repo> cargo test -p gitloom --release m5_timings -- --ignored --nocapture`
and `GITLOOM_PERF_REPO=<repo> cargo test -p git-core --release --test perf file_history_and_blame -- --ignored --nocapture`.

| Metric | Target | Generated 100 000-commit repository | This repository (293 commits) |
|---|---|---|---|
| Commit search | < 50 ms | **7.6–15.5 ms** (5 queries) | 0.03 ms |
| Hide / solo toggle incl. layout | < 200 ms | **147 ms** hide, **79 ms** solo, show all again 0.02 ms (the unfiltered snapshot is kept) | 0.2 ms |
| Palette filter, 100 000 paths | < 30 ms | **4.2–4.6 ms** (paths of the repository repeated) | 5.4–7.8 ms |
| File history | < 2 s for 2 000 commits | **0.52 s** (`hot.txt`, 2 000 commits) | 12 ms (`app.rs`, 69 commits) |
| Blame | < 2 s for 10 000 lines | **0.63 s** (`big.txt`, 10 000 lines) | 94 ms (`app.rs`, 11 286 lines) |
| Blame scrolling | smooth | 15 ms median, 19 ms max per wheel step (10 686 lines, X11 screenshot polling) | |

Synthetic unit timings (no repository): commit search over 100 000 commits with bodies
8.7–12.4 ms (`search_speed`), hide toggle 66 ms (`filter_speed`), palette filter 3.5–8.1 ms
(`palette_filter_speed`), all `cargo test --release -p gitloom <name> -- --ignored --nocapture`.
Time markers (M14-06, `perf_time_markers`): 60 visible rows 10.7 µs per frame (only visible
rows are built); all 100 000 rows at once 16 ms. The 100k layout benchmark is unchanged
(markers are drawn per visible row, the layout does not know them).
Narrow Graph column (M15-05): the overflow slot is decided once per frame from the column
width and the lane count; per visible row it only moves the node and skips or shortens
segments, `git-graph` is untouched. `cargo bench -p git-graph` on the M15 cloud container
(slower than the reference machine above): topo 8.9 ms, layout 10.7 ms, both 19.3 ms; the
100k layout benchmark is unchanged by construction.

### Idle memory with avatars

App started on the generated repository (release build), sampled from
`/proc/<pid>/smaps_rollup` after 15 s idle:

| Avatars | RSS | Anonymous | Network |
|---|---|---|---|
| Off | 307 MB | 117 MB | no TCP socket at all (sampled every 50 ms) |
| On (50 authors, all misses, cached) | 313 MB | 120 MB | requests to gravatar.com, 4 at a time |

## M7: diff view modes (task M7-05)

Cloud container (4 vCPU Intel Xeon @ 2.30 GHz, Linux), release build, no GPU: a frame is measured as building the view and laying it out with `iced_test` (what iced does per redraw before drawing). File: 50 000 lines, 1.95 MB, every 100th line changed (500 blocks).

| Mode | Open (diff + rows) | Rows | Frame median | Frame max (60 positions over the file) |
|---|---|---|---|---|
| Hunk | 51 ms | 4 500 | 6.1 ms | 16.5 ms |
| Inline | 45 ms | 51 000 | 6.3 ms | 18.7 ms |
| Split | 43 ms | 50 500 | 8.4 ms | 10.8 ms |

Target (milestone acceptance): no frame over 50 ms while scrolling a 2 MB file in Inline and Split; met. The row list is built once per load; a frame builds only the visible rows plus overscan, and the change navigation scans the block list only. No hot spot needed a fix. How: `cargo test -p gitloom --release --bin gitloom perf_large_diff_modes -- --ignored --nocapture`.

M7-16 (horizontal scrolling, 2026-11-10, same container): the measurement now runs with the text scrolled 120 px to the right. Hunk open 51–58 ms, frame median 7.2–7.6 ms, max 10.0–11.2 ms; Inline open 57 ms, median 7.5–7.6 ms, max 9.2–19.4 ms; Split open 49–53 ms, median 8.0–10.6 ms, max 15.9–18.6 ms (two runs). The target of 50 ms per frame is met; the width of the longest line is measured once per load.

## M11: syntax highlighting and word wrap (task M11-10)

Release build in the Linux container (2026-11-11), `cargo test -p gitloom --release perf_highlight_and_wrap -- --ignored --nocapture`.

| Metric | Target | Result |
|---|---|---|
| Highlight a 1 MB Rust file (11 554 lines of ≈ 88 characters), background task | – (UI never waits) | **3.4 s**; the diff shows plain at once and colors when it is done. Typical source files (< 100 KB) take well under 0.4 s. |
| File over the limit (2 MB) | plain at once | **0.2 µs** to decide "Too large for syntax highlighting" |
| Diff of a 2 MB file (every line changed), full context | – | **11 ms** |
| Load the wrapped 2 MB diff (46 213 rows: rows, character counts) | – | **12 ms** |
| Frame of the wrapped 2 MB diff (view + layout of the whole window), worst of 20 scroll positions | < 50 ms | **12 ms** |
| Word-wrap layout of 50 000 rows (prefix sums + visible rows) | < 50 ms | < 25 ms in a debug build (unit test `large_files_stay_fast`) |
| Binary size, `dist` profile, Linux x64 | – | **31.5 MB** (28.3 MB before M11-08: +3.2 MB for syntect, the regex engine and the syntax and theme sets) |

Highlighting uses syntect's pure Rust regex engine (`regex-fancy`, ADR-047), which is slower than oniguruma but needs no C library; whole files are highlighted once per content and theme and cached (64 entries).

## M13: Git flow layout (task M13-05)

Release build in the Linux container (2026-11-12), `cargo test --release -p git-graph flow_perf -- --ignored --nocapture`, on a generated Git flow history: 100 004 commits, features of 2–6 commits merged with Git's message, a release every 20 features into main and develop, a hotfix every 140; all topic branches deleted.

| Metric | Target | Result |
|---|---|---|
| Classify every commit (20 444 segments, none unknown) | – (background, snapshot step) | **19 ms** |
| Default layout | – | **6 ms** (4 lanes) |
| Git flow layout with lane groups (two passes) | – | **12 ms** (5 lanes) |
| Frame while scrolling | < 50 ms | unchanged: the view draws the same rows; only lane positions and colors differ |

Switching the layout lays out the loaded history again in the background (no reload); the classification runs on the filtered history when branches are hidden.

## Memory breakdown (rust-lang/rust)

| Part | Size |
|---|---|
| Empty app (start screen) | 122 MB RSS: 26 MB heap, 80 MB mapped libraries and fonts, 16 MB shared |
| Commit texts (summary, body, author, email) | 28 MB |
| Lane layout | ≈ 5 MB (runs + per-row curves; was 142 MB as one segment per lane and row, see ADR-011) |
| gix pack files mapped by the object database | 208 MB file-backed RSS (page cache, reclaimable by the OS) |

The memory target counts private memory (heap and anonymous mappings, ADR-012): 146 MB, target met. Memory-mapped pack files (208 MB, file-backed page cache) are listed separately and not counted. After a refresh the heap can stay higher (up to ≈ 250 MB observed, still within the target) because the allocator keeps memory of the previous snapshot.

## Changes made in M1-15

- Lane layout stores vertical runs instead of a `Through` segment per row (ADR-011): heap 337 MB → 210 MB, layout also faster.
- gix object cache per call reduced from 64 MB to 16 MB: heap 210 MB → 146 MB with unchanged log time (4 MB would cost ~5 % log time).

## Follow-up proposals

1. **Pack mappings** (optional, not part of the target): check whether gix can release or limit mapped pack data after loading.
2. **Allocator retention after refresh**: evaluate returning memory after a snapshot swap (e.g. a different global allocator); needs a decision because it adds a dependency.
3. ~~**Incremental refresh**~~: done in M2-05 (status-only refresh reuses the history).

Items 1 and 2 were checked in M2-16 and stay open: neither is cheap (gix has no simple switch to unmap packs after loading; a different allocator is a new dependency and needs a decision).
