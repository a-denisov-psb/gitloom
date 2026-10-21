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
3. **Incremental refresh** (with the file watcher in M2): reuse the commit list when only the working tree changed, instead of rebuilding the full snapshot on every window focus.
