# Architecture

State: end of M0, with the M1 target design. Sections marked _(M1)_ describe the design M1 implements.

## Crates

```text
            +-----------+
            |    app    |  iced UI, binary "gitloom"
            +-----------+
              |       |
              v       v
     +----------+   +-----------+
     | git-core |   | git-graph |   (M1)
     +----------+   +-----------+
          |
          v
         gix (M1), git CLI (M2)
```

| Crate | Responsibility | Must not depend on |
|---|---|---|
| `git-core` | Domain types (`Commit`, `Reference`, `FileChange`, ...), `GitBackend` trait, backends | iced |
| `git-graph` _(M1)_ | Topological order and lane layout; pure functions on plain data | gix, iced |
| `app` | State, messages, views, theme, persistence | gix (only through `git-core`) |

## Backends

| Backend | Used for | Milestone |
|---|---|---|
| `MockBackend` | Tests and `mock` feature only | M0 |
| `GixBackend` | All reads: log, refs, status, diffs | M1 |
| `CliBackend` | All writes: commit, checkout, fetch, push, merge, rebase | M2+ |

Rationale: `docs/DECISIONS.md` ADR-002. Writes through the CLI keep hooks, signing, credential helpers and SSH config behaving exactly like plain Git.

The read trait stays separate from the write trait so a read-only setup (no `git` installed) remains possible.

## Threading _(M1)_

- `GixBackend` holds a `gix::ThreadSafeRepository`. Each call converts to a thread-local `gix::Repository`.
- The app holds backends as `Arc<dyn GitBackend>`.
- All backend calls run in `tokio::task::spawn_blocking` inside `Task::perform` (iced feature `tokio`).
- Results come back as messages, e.g. `Message::SnapshotLoaded(TabId, Result<RepoSnapshot, Error>)`.
- Each load carries a generation counter; results from outdated loads are dropped.

```text
user action -> Message -> App::update -> Task::perform(spawn_blocking(backend call))
                                              |
          Message::*Loaded(result) <----------+
```

## App state _(M1 target)_

```text
App
├─ tabs: Vec<Tab>, active
├─ panes: pane_grid::State<PaneKind>       shared layout
├─ session: Session                        persisted (M1-09)
└─ status: transient status text

Tab
├─ id, generation
└─ state: Start | Loading { path } | Ready(RepoView) | Failed { path, error }

RepoView
├─ backend: Arc<dyn GitBackend>
├─ snapshot: RepoSnapshot   commits (topo order), layout rows, refs, stashes, working tree, head
├─ selection: Wip | Commit(Oid)          by id, survives refresh
├─ center: Graph | Diff(DiffTarget)
├─ sidebar state (filter, collapsed sections)
└─ scroll state (offset, viewport height)
```

## Graph layout _(M1, crate `git-graph`)_

### Ordering

Kahn's algorithm over the loaded commits:

1. Count children of every commit inside the loaded set.
2. Ready set = commits without loaded children (tips), as a max-heap by commit time; ties by input order.
3. Pop, emit, decrement each parent's child count; push parents that reach zero.

Result: children always before parents, newest first otherwise (equivalent to `git log --date-order`). Clock skew cannot break topology.

### Lanes

State: `lanes: Vec<Option<Slot>>`, `Slot { waiting_for: Id, color: ColorId }`.

For each commit `c` in order:

1. `incoming` = all lanes whose slot waits for `c`.
2. Node lane = lowest index in `incoming`; if empty (branch tip), the lowest free slot or a new lane at the end, with a new color.
3. Upper half segments:
   - each lane in `incoming` → `IntoNode { from: lane }` (straight if it is the node lane, curved otherwise),
   - every other occupied lane → `Through { lane }`.
4. Free all `incoming` lanes except the node lane.
5. Parents:
   - first parent: node lane now waits for it, same color → `FromNode { to: node_lane }`,
   - further parents: if a lane already waits for that parent, connect to it; otherwise take the lowest free slot (or append) with a new color → `FromNode { to: lane }`,
   - no parents (root): node lane becomes free.
6. Row width = highest occupied lane index in this row + 1.

Properties:

- Lanes never move sideways, only free slots are reused → stable, readable graphs.
- Two children of the same parent produce two lanes that join at the parent row (fork rendering).
- Missing parents (truncated history) keep their lane open to the bottom of the list.
- WIP is a virtual first row with HEAD as its only parent; its edges are `dashed`.

Output per row:

```rust
pub struct Row {
    pub lane: u16,
    pub color: ColorId,
    pub segments: Vec<Segment>,
    pub width: u16,
}

pub enum Segment {
    Through { lane: u16, color: ColorId },
    IntoNode { from: u16, color: ColorId, dashed: bool },
    FromNode { to: u16, color: ColorId, dashed: bool },
}
```

### Rendering

Each visible row draws its own segments into a canvas cell of the row height: upper half from the row top to the node center, lower half from the node center to the row bottom. Lane changes use cubic Bézier curves. Because every row is self-contained, virtualization needs no global canvas.

## Graph list virtualization _(M1)_

- Fixed row height (30 px).
- Visible range = `offset / row_height` … `(offset + viewport) / row_height` plus overscan.
- Column: top spacer, visible rows, bottom spacer, inside a scrollable with an `Id`.

## Persistence _(M1)_

`<config dir>/gitloom/session.json`, written atomically. Contains version, recent repositories, open tabs, active tab, pane ratios. See M1-09.

## Error handling

- `git_core::Error` is the only error type crossing the crate boundary; it carries a readable message and the source.
- No panics in non-test code. Errors are shown in the tab (banner) or as transient status text.

## Testing strategy

| Layer | Approach |
|---|---|
| `git-graph` | Unit + snapshot tests on hand-built histories, benchmark |
| `git-core` | Integration tests on temporary repos built with the `git` CLI |
| `app` | Unit tests of `update` logic and helpers; manual UI check per UI task |
