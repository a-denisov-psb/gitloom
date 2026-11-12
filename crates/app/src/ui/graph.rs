//! Center pane: commit table with the lane graph computed by `git-graph`.
//!
//! Every row draws its own slice of the graph (segments and node), so rows
//! can be rendered independently.

use git_core::{Commit, Head, RefKind, Reference};
use git_graph::{Row, Segment};
use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, LineDash, Path, Stroke};
use iced::widget::{
    button, column, container, responsive, row, scrollable, space, text, tooltip, Column,
};
use iced::{alignment, Color, Element, Fill, Length, Point, Rectangle, Renderer, Theme};

use crate::app::Message;
use crate::format;
use crate::graph_columns::{ColumnId, ColumnWidths};
use crate::icons::{self, Icon};
use crate::repo_view::{RepoView, Selection};
use crate::theme;
use crate::ui::muted;

pub const ROW_HEIGHT: f32 = 30.0;
const LANE_WIDTH: f32 = 18.0;
const GRAPH_PADDING: f32 = 6.0;
const ROW_PADDING: u16 = 8;
const SCROLLBAR_WIDTH: f32 = 12.0;
/// Height of the horizontal scrollbar strip below the table (M11-16).
const HBAR_HEIGHT: f32 = 10.0;
/// Room the table needs besides its columns: row paddings and scrollbar.
const FIXED_WIDTH: f32 = 2.0 * ROW_PADDING as f32 + SCROLLBAR_WIDTH;
const MIN_LANES: u16 = 2;
/// Narrowest Graph column: one lane (M11-02).
pub const MIN_GRAPH_WIDTH: f32 = GRAPH_PADDING * 2.0 + LANE_WIDTH;
/// Width of the fade at the right edge of a clipped Graph column.
const FADE_WIDTH: f32 = 12.0;
/// Horizontal padding of a table cell (both sides).
const CELL_PADDING: f32 = 4.0;
/// Header label size.
const HEADER_SIZE: f32 = 11.0;
const MAX_LANES: u16 = 12;
const NODE_RADIUS: f32 = 9.0;
const MERGE_RADIUS: f32 = 4.5;
/// Side of the stash square and of the icon inside it (M11-05).
const STASH_SIZE: f32 = 16.0;
const STASH_ICON: f32 = 10.0;
/// Graph line style like GitKraken's (M7-18, measured against a
/// reference screenshot; style tokens in M9): line width, the radius of
/// the rounded 90° turn where an edge leaves or joins a node, and the
/// dotted pattern of WIP and stash edges.
const LINE_WIDTH: f32 = 1.5;
const CORNER_RADIUS: f32 = 6.0;
const DASH: [f32; 2] = [2.0, 2.0];
/// Lines run this far past the top and bottom edge of their row. The canvas
/// clips them at the row edge, which the renderer snaps to physical pixels
/// the same way for both neighbours, so rows join without a gap at any
/// scale factor (M6-23). At least one physical pixel down to a scale of 0.5.
const EDGE_OVERLAP: f32 = 2.0;
/// Rows rendered above and below the viewport.
const OVERSCAN: usize = 10;

pub fn view<'a>(
    repo: &'a RepoView,
    avatars: &'a crate::avatars::Avatars,
    appearance: &crate::settings::Appearance,
    palette: &'static theme::ThemePalette,
) -> Element<'a, Message> {
    let lanes = repo.snapshot.layout.max_width;
    let widths = appearance.graph_columns.sanitized(MIN_GRAPH_WIDTH);
    let graph_width = widths.width(ColumnId::Graph, graph_width(lanes));
    // A Graph column narrower than the lanes fades out at its right edge.
    let clipped = graph_width < lanes_width(lanes);
    let columns = Columns::shown(appearance);
    let table = responsive(move |size| {
        // Every column has its own width (M11-16): the table is as wide as
        // its columns, the message fills a wider viewport, and a wider
        // table scrolls horizontally (header and rows together).
        let viewport = size.width;
        let layout = crate::graph_columns::layout(
            viewport,
            FIXED_WIDTH,
            columns.sum(&widths, graph_width),
            widths.message,
        );
        let message_width = Length::Fixed(layout.message);
        let header_cell = |label: &'static str, column: ColumnId| -> Element<'_, Message> {
            let width = match column {
                ColumnId::Message => layout.message,
                _ => widths.width(column, graph_width),
            };
            let handle = crate::ui::column_handle::column_handle(
                Message::ColumnDragStarted {
                    column,
                    origin: width,
                },
                Message::ColumnDragged,
                Message::ColumnDragEnded,
                Message::ColumnFit { column, viewport },
            );
            iced::widget::Stack::with_children([
                cell(muted(label).size(HEADER_SIZE), width),
                header_separator(width),
                container(handle)
                    .width(width)
                    .height(Fill)
                    .align_x(alignment::Horizontal::Right)
                    .into(),
            ])
            .width(width)
            .into()
        };
        // The header cells line up with the row cells; the buttons lie over
        // its right end like GitKraken's gear.
        let mut header: iced::widget::Row<'_, Message> = row![
            header_cell("BRANCH / TAG", ColumnId::Refs),
            header_cell("GRAPH", ColumnId::Graph),
            header_cell("COMMIT MESSAGE", ColumnId::Message),
        ];
        if columns.author {
            header = header.push(header_cell("AUTHOR", ColumnId::Author));
        }
        if columns.date {
            header = header.push(header_cell("DATE", ColumnId::Date));
        }
        if columns.sha {
            header = header.push(header_cell("SHA", ColumnId::Sha));
        }
        // Above the vertical scrollbar's room of the rows.
        header = header.push(space::horizontal().width(SCROLLBAR_WIDTH));
        let mut buttons = row![].align_y(iced::Center);
        if repo.search.is_none() {
            let tip = match crate::actions::ActionId::SearchCommits.shortcut_text() {
                Some(keys) => format!("Search commits ({keys})"),
                None => "Search commits".to_owned(),
            };
            buttons = buttons.push(crate::ui::panel::with_tooltip_text(
                iced::widget::button(icons::tinted(Icon::Search, 13.0, |p| p.text_muted))
                    .on_press(Message::SearchOpened)
                    .padding([2, 3])
                    .style(theme::flat)
                    .into(),
                tip,
            ));
        }
        buttons = buttons.push(crate::ui::panel::with_tooltip_text(
            crate::ui::context_area::click_area(
                iced::widget::mouse_area(
                    container(icons::tinted(Icon::Settings, 13.0, |p| p.text_muted))
                        .padding([2, 3]),
                )
                .interaction(iced::mouse::Interaction::Pointer),
                Message::GraphColumnMenu,
            ),
            "Columns".to_owned(),
        ));
        // Header and rows move by the same offset (M11-16).
        let offset = repo.graph_h_offset.min(layout.max_offset);
        let header_row = crate::ui::shift::shift(
            container(header.padding([0, ROW_PADDING]).align_y(iced::Center))
                .width(layout.table)
                .height(24)
                .align_y(alignment::Vertical::Center),
            offset,
        );
        let header = iced::widget::Stack::with_children([
            header_row,
            container(
                container(buttons)
                    .padding([0, 2])
                    .height(Fill)
                    .align_y(alignment::Vertical::Center)
                    .style(theme::column_header),
            )
            .width(Fill)
            .height(24)
            .align_x(alignment::Horizontal::Right)
            .into(),
        ]);
        let header = container(header)
            .height(24)
            .width(Fill)
            .clip(true)
            .style(theme::column_header);

        // Only the visible rows (plus overscan) exist; spacers keep the scrollbar exact.
        // A truncated history ends with a note row.
        let graph_rows = repo.snapshot.layout.len();
        let total = graph_rows + usize::from(repo.snapshot.truncated);
        let (first, last) = visible_range(repo.scroll.offset, repo.scroll.height, total);
        let mut rows = Column::with_capacity(last - first + 2)
            .push(space::vertical().height(first as f32 * ROW_HEIGHT));
        for row_index in first..last {
            rows = rows.push(if row_index < graph_rows {
                graph_row(
                    repo,
                    avatars,
                    row_index,
                    &RowLayout {
                        widths: &widths,
                        graph_width,
                        clipped,
                        columns,
                        message_width,
                        palette,
                    },
                )
            } else {
                truncated_row()
            });
        }
        rows = rows.push(space::vertical().height((total - last) as f32 * ROW_HEIGHT));

        // The vertical scrollbar keeps its room at the right edge of the
        // viewport (M6-22) while the rows move sideways (M11-16).
        let rows = crate::ui::shift::shift(
            container(rows).width(layout.table - SCROLLBAR_WIDTH),
            offset,
        );
        let list = scrollable(rows)
            .id(repo.graph_scroll_id.clone())
            .spacing(crate::ui::SCROLLBAR_GAP)
            .on_scroll(|viewport| Message::GraphScrolled {
                offset: viewport.absolute_offset().y,
                height: viewport.bounds().height,
            })
            .height(Fill);
        // Shift+wheel and a horizontal wheel scroll sideways like the diffs.
        let list = crate::ui::context_area::wheel_observer(list, |delta, shift| {
            crate::diff_hscroll::wheel_to_horizontal(delta, shift, cfg!(target_os = "macos"))
                .map(Message::GraphWheel)
        });
        let mut table = match &repo.search {
            Some(search) => column![search_bar(search), header],
            None => column![header],
        };
        // A running operation: a row in the warning color above the
        // commits (M12-05); a click shows the WIP panel.
        if let Some(status) = repo.operation() {
            table = table.push(operation_row(status.row));
        }
        let mut table = table.push(list);
        if layout.max_offset > 0.0 {
            table = table.push(horizontal_bar(repo, viewport, layout.max_offset));
        }
        table.into()
    });

    container(table)
        .width(Fill)
        .height(Fill)
        .style(theme::graph)
        .into()
}

/// Horizontal scrollbar below a table wider than its viewport (M11-16):
/// it reports moves and follows wheel scrolling.
fn horizontal_bar(repo: &RepoView, viewport: f32, max: f32) -> Element<'_, Message> {
    scrollable(
        space::horizontal()
            .width(viewport + max)
            .height(HBAR_HEIGHT),
    )
    .direction(scrollable::Direction::Horizontal(
        scrollable::Scrollbar::new(),
    ))
    .id(repo.graph_hscroll_id.clone())
    .on_scroll(|viewport| Message::GraphHScrolled {
        offset: viewport.absolute_offset().x,
        max: viewport.content_bounds().width - viewport.bounds().width,
    })
    .width(Fill)
    .height(HBAR_HEIGHT)
    .into()
}

/// Commit search above the graph (M5-07): query, "n of m", previous/next,
/// close.
fn search_bar(search: &crate::search::SearchState) -> Element<'_, Message> {
    let button = |icon: Icon, message: Message, tip: &str, enabled: bool| {
        crate::ui::panel::with_tooltip_text(
            iced::widget::button(icons::button_icon(icon, 14.0, enabled))
                .on_press_maybe(enabled.then_some(message))
                .padding([3, 6])
                .style(theme::flat)
                .into(),
            tip.to_owned(),
        )
    };
    let any = !search.matches.is_empty();
    let content = row![
        icons::tinted(Icon::Search, 14.0, |p| p.text_muted),
        iced::widget::text_input(
            "Search commits: message, author, email or SHA",
            &search.query
        )
        .id(crate::app::SEARCH_INPUT)
        .on_input(Message::SearchInput)
        .on_submit(Message::SearchSubmit)
        .padding([4, 8])
        .style(theme::input)
        .size(13),
        muted(search.counter()).size(12).width(70),
        button(
            Icon::ChevronUp,
            Message::SearchStep { forward: false },
            "Previous match (Shift+Enter)",
            any
        ),
        button(
            Icon::ChevronDown,
            Message::SearchStep { forward: true },
            "Next match (Enter)",
            any
        ),
        button(
            Icon::Close,
            Message::SearchClosed,
            "Close search (Esc)",
            true
        ),
    ]
    .spacing(6)
    .padding([4, ROW_PADDING])
    .align_y(iced::Center);
    container(content)
        .width(Fill)
        .style(theme::column_header)
        .into()
}

/// "A file conflict was found when attempting to merge into main".
fn operation_row<'a>(label: String) -> Element<'a, Message> {
    let content = container(
        row![
            icons::tinted(Icon::Alert, 14.0, |p| p.warning),
            text(label).size(13).wrapping(text::Wrapping::None),
        ]
        .spacing(8)
        .align_y(alignment::Vertical::Center),
    )
    .height(ROW_HEIGHT)
    .width(Fill)
    .clip(true)
    .padding([0, 12])
    .align_y(alignment::Vertical::Center)
    .style(theme::operation_row);
    iced::widget::mouse_area(content)
        .on_press(Message::WipSelected)
        .interaction(mouse::Interaction::Pointer)
        .into()
}

fn truncated_row<'a>() -> Element<'a, Message> {
    container(muted(format!(
        "History truncated at {} commits",
        format::thousands(crate::snapshot::LOG_LIMIT)
    )))
    .height(ROW_HEIGHT)
    .width(Fill)
    .padding([0, 16])
    .align_y(alignment::Vertical::Center)
    .into()
}

/// Optional columns shown next to the commit message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Columns {
    pub author: bool,
    pub date: bool,
    pub sha: bool,
}

impl Columns {
    /// The optional columns the settings show (gear menu, Preferences →
    /// Appearance; M11-03).
    pub fn shown(appearance: &crate::settings::Appearance) -> Self {
        Self {
            author: appearance.graph_author,
            date: appearance.graph_date,
            sha: appearance.graph_sha,
        }
    }

    /// Summed width of Branch/Tag, Graph and the shown optional columns.
    pub fn sum(&self, widths: &ColumnWidths, graph_width: f32) -> f32 {
        widths.refs
            + graph_width
            + if self.author { widths.author } else { 0.0 }
            + if self.date { widths.date } else { 0.0 }
            + if self.sha { widths.sha } else { 0.0 }
    }
}

/// Ref pill geometry, shared by [`pill`] and [`pills_width`].
const PILL_PADDING: f32 = 6.0;
const PILL_TEXT_SIZE: f32 = 12.0;
const PILL_ICON_SIZE: f32 = 11.0;
const REMOTE_ICON_SIZE: f32 = 12.0;
const PILL_SPACING: f32 = 4.0;
/// Size of the "+N" after the first pill.
const MORE_SIZE: f32 = 11.0;

/// Default text size of the table (the app's default).
const TEXT_SIZE: f32 = 13.0;
/// Slack added to measured widths so rounding never clips the text.
const FIT_SLACK: f32 = 2.0;

/// Optimal width of `column` (double click on its border, M11-02): the
/// widest content in the visible rows; Graph: all lanes of the loaded
/// layout, at most half of the `table` width. Not limited here; the caller
/// applies minimum and room.
pub fn optimal_column_width(repo: &RepoView, column: ColumnId, table: f32) -> f32 {
    use crate::overlay::{text_width, text_width_in};
    if column == ColumnId::Graph {
        return crate::graph_columns::graph_optimal(
            lanes_width(repo.snapshot.layout.max_width),
            table,
        );
    }
    let snapshot = &repo.snapshot;
    let wip = usize::from(snapshot.has_wip);
    // Before the first scroll event the height is unknown: a typical window.
    let height = if repo.scroll.height > 0.0 {
        repo.scroll.height
    } else {
        40.0 * ROW_HEIGHT
    };
    let (first, last) = crate::ui::rows_in_view(
        repo.scroll.offset,
        height,
        snapshot.layout.len(),
        ROW_HEIGHT,
        0,
    );
    let commits = (first.saturating_sub(wip)..last.saturating_sub(wip))
        .filter_map(|index| snapshot.commits.get(index).map(|commit| (index, commit)));
    let content = commits
        .map(|(index, commit)| match column {
            ColumnId::Refs => {
                let refs: Vec<&Reference> = snapshot.refs_at(index).collect();
                pills_width(&ref_labels(&refs, &snapshot.head, commit))
            }
            ColumnId::Author => text_width(&commit.author, TEXT_SIZE),
            ColumnId::Date => text_width(&format::date_time(commit.author_time), TEXT_SIZE),
            ColumnId::Sha => text_width_in(
                &commit.id[..commit.id.len().min(7)],
                TEXT_SIZE,
                iced::Font::MONOSPACE,
            ),
            ColumnId::Message => text_width(&commit.summary, TEXT_SIZE),
            ColumnId::Graph => 0.0,
        })
        .fold(0.0_f32, f32::max);
    content + 2.0 * CELL_PADDING + FIT_SLACK
}

/// Width of the pill group [`ref_pills`] draws for `labels`.
fn pills_width(labels: &[RefLabel]) -> f32 {
    use crate::overlay::text_width;
    let Some(first) = labels.first() else {
        return 0.0;
    };
    // Padding 6 on both sides and the 1 px border.
    let mut pill = 2.0 * PILL_PADDING + 2.0 + text_width(&first.text, PILL_TEXT_SIZE);
    if first.head && first.kind == LabelKind::Local {
        pill += PILL_ICON_SIZE + PILL_SPACING;
    }
    if !first.remotes.is_empty() {
        pill += REMOTE_ICON_SIZE + PILL_SPACING;
    }
    if labels.len() > 1 {
        pill += PILL_SPACING + text_width(&format!("+{}", labels.len() - 1), MORE_SIZE);
    }
    pill
}

/// Rows `first..last` intersecting the viewport, extended by [`OVERSCAN`].
pub fn visible_range(offset: f32, height: f32, total: usize) -> (usize, usize) {
    crate::ui::rows_in_view(offset, height, total, ROW_HEIGHT, OVERSCAN)
}

/// New scroll offset that brings `row` fully into view, if it is not visible.
pub fn reveal_offset(row: usize, offset: f32, height: f32) -> Option<f32> {
    let top = row as f32 * ROW_HEIGHT;
    let bottom = top + ROW_HEIGHT;
    if top < offset {
        Some(top)
    } else if bottom > offset + height {
        Some((bottom - height).max(0.0))
    } else {
        None
    }
}

/// Width of the graph column for `lanes` lanes (clamped to 2..=12).
pub fn graph_width(lanes: u16) -> f32 {
    let lanes = lanes.clamp(MIN_LANES, MAX_LANES);
    GRAPH_PADDING * 2.0 + LANE_WIDTH * f32::from(lanes)
}

/// Width all `lanes` need (at least one): the optimal Graph width.
pub fn lanes_width(lanes: u16) -> f32 {
    GRAPH_PADDING * 2.0 + LANE_WIDTH * f32::from(lanes.max(1))
}

/// Column geometry shared by all rows of one frame.
#[derive(Clone, Copy)]
pub struct RowLayout<'w> {
    pub widths: &'w ColumnWidths,
    pub graph_width: f32,
    /// The Graph column is narrower than the lanes (fade).
    pub clipped: bool,
    pub columns: Columns,
    /// Fill, or the minimum when the window is too narrow.
    pub message_width: Length,
    /// Palette of the active theme (style tokens, M11-04).
    pub palette: &'static theme::ThemePalette,
}

/// One table row: the WIP row or a commit row.
pub fn graph_row<'a>(
    repo: &'a RepoView,
    avatars: &crate::avatars::Avatars,
    row_index: usize,
    layout: &RowLayout<'_>,
) -> Element<'a, Message> {
    let RowLayout {
        widths,
        graph_width,
        clipped,
        columns,
        message_width,
        palette,
    } = *layout;
    let snapshot = &repo.snapshot;
    let Some(layout_row) = snapshot.layout.row(row_index) else {
        return container(text("")).height(ROW_HEIGHT).into();
    };
    let color = layout_row.color.0;

    if snapshot.has_wip && row_index == 0 {
        let tree = &snapshot.working_tree;
        let count = |icon: Icon, n: usize| {
            row![
                icons::tinted(icon, 12.0, |p| p.text_muted),
                muted(n.to_string())
            ]
            .spacing(4)
            .align_y(iced::Center)
        };
        let summary = row![
            muted("// WIP"),
            count(Icon::Pencil, tree.unstaged.len()),
            count(Icon::Check, tree.staged.len()),
        ]
        .spacing(12)
        .align_y(iced::Center);
        let refs: Element<'_, Message> = match &repo.new_branch {
            Some(input) if input.row.is_none() => new_branch_input(input),
            _ => text("").into(),
        };
        let content = row![
            cell(refs, widths.refs),
            graph_column(
                graph_cell(layout_row, row_index, Node::Wip, false, CellLook::default()),
                graph_width,
                clipped,
                repo.selection == Some(Selection::Wip),
                color,
            ),
            cell(summary, message_width),
        ];
        return row_button(
            content,
            repo.selection == Some(Selection::Wip),
            color,
            Message::WipSelected,
        );
    }

    let index = row_index - usize::from(snapshot.has_wip);
    let Some(commit) = snapshot.commits.get(index) else {
        return container(text("")).height(ROW_HEIGHT).into();
    };
    let is_stash = snapshot.is_stash(&commit.id);
    // Rows that do not match an active search are dimmed (M5-07).
    let dimmed = repo.search.as_ref().is_some_and(|s| s.dims(index));
    let node = if is_stash {
        Node::Stash
    } else if commit.parents.len() > 1 {
        Node::Merge
    } else {
        Node::Commit {
            initials: format::initials(&commit.author),
            avatar: avatars.get(&commit.email).cloned(),
        }
    };
    let summary = if dimmed {
        text(&commit.summary).style(theme::text_faint)
    } else if commit.parents.len() > 1 {
        muted(&commit.summary)
    } else {
        text(&commit.summary)
    };

    let selected =
        repo.selected_commit() == Some(commit.id.as_str()) || repo.multi.contains(&commit.id);
    let refs: Vec<&Reference> = snapshot.refs_at(index).collect();
    let labels = ref_labels(&refs, &snapshot.head, commit);
    let look = CellLook {
        tint: !selected,
        connector: !labels.is_empty(),
    };
    let refs_cell: Element<'_, Message> = match &repo.new_branch {
        Some(input) if input.row.as_deref() == Some(commit.id.as_str()) => {
            cell(new_branch_input(input), widths.refs)
        }
        _ if look.connector && palette.ref_connector => {
            // Classic Dark: the pill's line continues to the column edge
            // and on in the graph cell to the node (M11-04).
            let connector = container(space::horizontal())
                .width(Fill)
                .height(LINE_WIDTH)
                .style(theme::ref_connector(color));
            container(
                row![ref_pills(labels, repo.drag.as_ref(), color), connector].align_y(iced::Center),
            )
            .width(widths.refs)
            .height(Fill)
            .padding(iced::Padding::default().left(CELL_PADDING))
            .align_y(alignment::Vertical::Center)
            .clip(true)
            .into()
        }
        _ => cell(ref_pills(labels, repo.drag.as_ref(), color), widths.refs),
    };
    let mut content = row![
        refs_cell,
        graph_column(
            graph_with_avatar(layout_row, row_index, node, dimmed, look),
            graph_width,
            clipped,
            selected,
            color,
        ),
        cell(summary.wrapping(text::Wrapping::None), message_width),
    ];
    if columns.author {
        content = content.push(cell(
            muted(&commit.author).wrapping(text::Wrapping::None),
            widths.author,
        ));
    }
    if columns.date {
        content = content.push(cell(
            muted(format::date_time(commit.author_time)).wrapping(text::Wrapping::None),
            widths.date,
        ));
    }
    if columns.sha {
        content = content.push(cell(
            muted(&commit.id[..commit.id.len().min(7)]).font(iced::Font::MONOSPACE),
            widths.sha,
        ));
    }

    let row = row_button(
        content,
        selected,
        color,
        Message::CommitSelected(commit.id.clone()),
    );
    let id = commit.id.clone();
    if is_stash {
        crate::ui::context_area::context_area(row, move |position| Message::StashContextMenu {
            id: id.clone(),
            position,
        })
    } else {
        crate::ui::context_area::context_area(row, move |position| Message::CommitContextMenu {
            id: id.clone(),
            position,
        })
    }
}

/// Inline input for a new branch (GitKraken); an error shows as red border
/// and tooltip.
fn new_branch_input(input: &crate::repo_view::NewBranch) -> Element<'_, Message> {
    let field = iced::widget::text_input("New branch name", &input.name)
        .id(crate::ui::NEW_BRANCH_INPUT)
        .on_input(Message::NewBranchInput)
        .on_submit(Message::NewBranchSubmit)
        .size(12)
        .padding([1, 4])
        .style(theme::inline_input(input.error.is_some()));
    match &input.error {
        Some(error) => tooltip(
            field,
            container(text(error.clone()).size(12))
                .padding([4, 8])
                .style(theme::tooltip),
            tooltip::Position::Bottom,
        )
        .into(),
        None => field.into(),
    }
}

/// What a reference pill shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefLabel {
    /// Branch or tag name ("HEAD" for a detached HEAD).
    pub text: String,
    /// Remotes with a same-named branch on the same commit (combined pill).
    pub remotes: Vec<String>,
    pub kind: LabelKind,
    /// The checked-out branch (or the detached HEAD marker).
    pub head: bool,
    /// Double click on the pill checks this out.
    pub checkout: Option<crate::app::CheckoutTarget>,
}

impl RefLabel {
    /// Name plus the remotes of a combined pill: "main (origin, upstream)".
    pub fn description(&self) -> String {
        if self.remotes.is_empty() {
            self.text.clone()
        } else {
            format!("{} ({})", self.text, self.remotes.join(", "))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    /// Local branch, possibly combined with same-named remote branches.
    Local,
    Remote,
    Tag,
    /// `HEAD` pointing directly at this commit.
    DetachedHead,
}

/// Groups the references of one commit like GitKraken: a local branch and
/// remote branches with the same name share one pill; HEAD comes first.
pub fn ref_labels(refs: &[&Reference], head: &Head, commit: &Commit) -> Vec<RefLabel> {
    let mut labels = Vec::new();
    if let Head::Detached { target } = head {
        if *target == commit.id {
            labels.push(RefLabel {
                text: "HEAD".into(),
                remotes: Vec::new(),
                kind: LabelKind::DetachedHead,
                head: true,
                checkout: None,
            });
        }
    }

    let mut used = vec![false; refs.len()];
    let mut locals: Vec<(usize, &Reference)> = refs
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == RefKind::LocalBranch)
        .map(|(i, r)| (i, *r))
        .collect();
    locals.sort_by_key(|(_, r)| !r.is_head);
    for (i, local) in locals {
        used[i] = true;
        let mut remotes = Vec::new();
        for (j, remote) in refs.iter().enumerate() {
            if !used[j] && remote.kind == RefKind::RemoteBranch && remote.local_name() == local.name
            {
                used[j] = true;
                remotes.extend(remote.remote().map(str::to_owned));
            }
        }
        labels.push(RefLabel {
            text: local.name.clone(),
            remotes,
            kind: LabelKind::Local,
            head: local.is_head,
            checkout: Some(crate::app::CheckoutTarget::Local(local.name.clone())),
        });
    }
    for kind in [RefKind::RemoteBranch, RefKind::Tag] {
        for (j, reference) in refs.iter().enumerate() {
            if !used[j] && reference.kind == kind {
                used[j] = true;
                labels.push(RefLabel {
                    text: reference.name.clone(),
                    remotes: Vec::new(),
                    kind: if kind == RefKind::Tag {
                        LabelKind::Tag
                    } else {
                        LabelKind::Remote
                    },
                    head: false,
                    checkout: (kind == RefKind::RemoteBranch)
                        .then(|| crate::app::CheckoutTarget::Remote(reference.name.clone())),
                });
            }
        }
    }
    labels
}

/// First label as pill; the rest collapse into "+N" with a tooltip listing them.
fn ref_pills<'a>(
    labels: Vec<RefLabel>,
    drag: Option<&crate::repo_view::BranchDrag>,
    lane: u32,
) -> Element<'a, Message> {
    let Some(first) = labels.first() else {
        return text("").into();
    };
    let mut pills = row![pill(first, drag, lane)]
        .spacing(PILL_SPACING)
        .align_y(iced::Center);
    if labels.len() > 1 {
        let rest = Column::with_children(
            labels[1..]
                .iter()
                .map(|label| text(label.description()).size(12).into()),
        )
        .spacing(2);
        pills = pills.push(tooltip(
            muted(format!("+{}", labels.len() - 1)).size(MORE_SIZE),
            container(rest).padding([4, 8]).style(theme::tooltip),
            tooltip::Position::Bottom,
        ));
    }
    pills.into()
}

fn pill<'a>(
    label: &RefLabel,
    drag: Option<&crate::repo_view::BranchDrag>,
    lane: u32,
) -> Element<'a, Message> {
    let kind = match label.kind {
        LabelKind::Local => RefKind::LocalBranch,
        LabelKind::Remote => RefKind::RemoteBranch,
        LabelKind::Tag => RefKind::Tag,
        LabelKind::DetachedHead => RefKind::LocalBranch,
    };
    let mut content = row![].spacing(PILL_SPACING).align_y(iced::Center);
    if label.head && label.kind == LabelKind::Local {
        content = content.push(icons::tinted(Icon::Check, PILL_ICON_SIZE, theme::pill_icon));
    }
    content = content.push(
        text(label.text.clone())
            .size(PILL_TEXT_SIZE)
            .wrapping(text::Wrapping::None),
    );
    if !label.remotes.is_empty() {
        // GitKraken shows a remote icon on combined pills; the names are
        // in the tooltip.
        content = content.push(tooltip(
            icons::tinted(Icon::Cloud, REMOTE_ICON_SIZE, theme::pill_icon_muted),
            container(text(label.remotes.join(", ")).size(12))
                .padding([4, 8])
                .style(theme::tooltip),
            tooltip::Position::Bottom,
        ));
    }
    let pill = container(content)
        .padding([1.0, PILL_PADDING])
        .clip(true)
        .style(theme::ref_pill_in(kind, label.head, Some(lane)));
    let pill: Element<'_, Message> = match &label.checkout {
        Some(target) => {
            crate::ui::context_area::double_click_area(pill, Message::Checkout(target.clone()))
        }
        None => pill.into(),
    };
    // Right click on a branch pill opens the branch menu, not the commit
    // menu; branch pills can be dragged onto each other (M4-11).
    let branch = |pill: Element<'a, Message>, name: &str, kind: RefKind| {
        let menu_name = name.to_owned();
        let pill = crate::ui::context_area::context_area(pill, move |position| {
            Message::BranchContextMenu {
                name: menu_name.clone(),
                kind,
                position,
            }
        });
        let highlight = drag.is_some_and(|d| d.targets(name, kind));
        crate::ui::draggable_branch(pill, name, kind, highlight)
    };
    match &label.checkout {
        Some(crate::app::CheckoutTarget::Local(name)) => branch(pill, name, RefKind::LocalBranch),
        Some(crate::app::CheckoutTarget::Remote(name)) => branch(pill, name, RefKind::RemoteBranch),
        _ if label.kind == LabelKind::Tag => {
            let name = label.text.clone();
            crate::ui::context_area::context_area(pill, move |position| {
                Message::BranchContextMenu {
                    name: name.clone(),
                    kind: RefKind::Tag,
                    position,
                }
            })
        }
        _ => pill,
    }
}

/// Line at the right edge of a header cell `width` wide (the
/// `column_separator` token: Classic Dark only, M11-04).
fn header_separator<'a>(width: impl Into<Length>) -> Element<'a, Message> {
    container(
        container(space::vertical())
            .width(1)
            .height(14)
            .style(theme::column_separator),
    )
    .width(width)
    .height(Fill)
    .align_x(alignment::Horizontal::Right)
    .align_y(alignment::Vertical::Center)
    .into()
}

/// Graph cell `width` wide, with a fade at its right edge when it clips the
/// lanes (M11-02).
fn graph_column(
    graph: Element<'_, Message>,
    width: f32,
    clipped: bool,
    selected: bool,
    lane_color: u32,
) -> Element<'_, Message> {
    // No left padding: the canvas starts at the column edge, so a ref
    // connector (Classic Dark) can reach it; `lane_x` adds the padding.
    let graph: Element<'_, Message> = container(graph)
        .width(width)
        .height(Fill)
        .padding(iced::Padding::default().right(CELL_PADDING))
        .align_y(alignment::Vertical::Center)
        .clip(true)
        .into();
    if !clipped {
        return graph;
    }
    iced::widget::Stack::with_children([
        graph,
        container(
            container(space::horizontal())
                .width(FADE_WIDTH)
                .height(Fill)
                .style(theme::graph_fade(selected, lane_color)),
        )
        .width(width)
        .height(Fill)
        .align_x(alignment::Horizontal::Right)
        .into(),
    ])
    .width(width)
    .into()
}

fn cell<'a>(
    content: impl Into<Element<'a, Message>>,
    width: impl Into<Length>,
) -> Element<'a, Message> {
    container(content)
        .width(width)
        .height(Fill)
        .padding([0.0, CELL_PADDING])
        .align_y(alignment::Vertical::Center)
        .clip(true)
        .into()
}

fn row_button<'a>(
    content: iced::widget::Row<'a, Message>,
    selected: bool,
    lane_color: u32,
    on_press: Message,
) -> Element<'a, Message> {
    button(content.padding([0, ROW_PADDING]).height(Fill))
        .on_press(on_press)
        .padding(0)
        .width(Fill)
        .height(ROW_HEIGHT)
        .style(theme::graph_row(selected, lane_color))
        .into()
}

#[derive(Debug, Clone)]
enum Node {
    Wip,
    Merge,
    /// Stash entry: hollow square.
    Stash,
    /// Regular commit: the author's avatar, or initials without one.
    Commit {
        initials: String,
        avatar: Option<iced::widget::image::Handle>,
    },
}

/// Graph slice of one row.
struct GraphCell {
    row: Row,
    /// Position of the row in the graph (dash phase across rows).
    index: usize,
    node: Node,
    /// The commit does not match the search: faint node.
    dimmed: bool,
    /// Classic Dark extras (M11-04, drawn only when the palette asks).
    look: CellLook,
}

/// What a graph cell may add in themes with the style tokens on (M11-04).
#[derive(Debug, Clone, Copy, Default)]
struct CellLook {
    /// Tint the row from the node to the right edge (`row_tint_alpha`);
    /// off for the selected row, whose own color stays on top.
    tint: bool,
    /// The row has ref pills: a line from the left edge to the node
    /// (`ref_connector`).
    connector: bool,
}

/// Graph cell; an avatar is an image widget on top of the node, because the
/// canvas does not round images (M5-11).
fn graph_with_avatar<'a>(
    row: Row,
    index: usize,
    node: Node,
    dimmed: bool,
    look: CellLook,
) -> Element<'a, Message> {
    let avatar = match &node {
        Node::Commit {
            avatar: Some(handle),
            ..
        } => Some(handle.clone()),
        _ => None,
    };
    let x = lane_x(row.lane);
    let lane = row.color.0;
    let stash = matches!(node, Node::Stash);
    let cell = graph_cell(row, index, node, dimmed, look);
    if stash {
        let alpha = if dimmed { 0.3 } else { 1.0 };
        let icon = icons::colored(Icon::Stash, STASH_ICON, move |theme| {
            theme::lane_color(theme::palette(theme), lane).scale_alpha(alpha)
        });
        return iced::widget::Stack::with_children([
            cell,
            container(icon)
                .padding(iced::Padding::default().left(x - STASH_ICON / 2.0))
                .height(Fill)
                .align_y(alignment::Vertical::Center)
                .into(),
        ])
        .into();
    }
    let Some(handle) = avatar else {
        return cell;
    };
    let size = NODE_RADIUS * 2.0;
    let image = iced::widget::image(handle)
        .width(size)
        .height(size)
        .border_radius(NODE_RADIUS)
        .opacity(if dimmed { 0.3_f32 } else { 1.0 });
    iced::widget::Stack::with_children([
        cell,
        container(image)
            .padding(iced::Padding::default().left(x - NODE_RADIUS))
            .height(Fill)
            .align_y(alignment::Vertical::Center)
            .into(),
    ])
    .into()
}

fn graph_cell<'a>(
    row: Row,
    index: usize,
    node: Node,
    dimmed: bool,
    look: CellLook,
) -> Element<'a, Message> {
    canvas::Canvas::new(GraphCell {
        row,
        index,
        node,
        dimmed,
        look,
    })
    .width(Fill)
    .height(Fill)
    .into()
}

/// Radius of the ring around an avatar (and, Classic Dark, around
/// initials, with a gap like GitKraken's).
fn ring_radius(p: &theme::ThemePalette) -> f32 {
    match p.node_style {
        theme::NodeStyle::Standard => NODE_RADIUS + 1.0,
        theme::NodeStyle::Ringed => NODE_RADIUS + 2.5,
    }
}

/// Center of `lane` in the graph canvas (which starts at the column edge).
fn lane_x(lane: u16) -> f32 {
    CELL_PADDING + GRAPH_PADDING + LANE_WIDTH * f32::from(lane) + LANE_WIDTH / 2.0
}

impl canvas::Program<Message> for GraphCell {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let p = theme::palette(theme);
        let mut frame = Frame::new(renderer, bounds.size());
        let height = bounds.height;
        let mid = height / 2.0;
        let node = Point::new(lane_x(self.row.lane), mid);
        let lane = theme::lane_color(p, self.row.color.0);

        // Classic Dark (M11-04): row tint from the node to the right edge,
        // and the line from the ref pill (left edge) to the node.
        if self.look.tint && p.row_tint_alpha > 0.0 && node.x < bounds.width {
            frame.fill_rectangle(
                Point::new(node.x, 0.0),
                iced::Size::new(bounds.width - node.x, height),
                lane.scale_alpha(p.row_tint_alpha),
            );
        }
        if self.look.connector && p.ref_connector {
            let path = Path::line(Point::new(0.0, mid), node);
            frame.stroke(&path, line(lane, false));
        }

        for segment in &self.row.segments {
            let (ops, color, dashed) = match *segment {
                Segment::Through {
                    lane,
                    color,
                    dashed,
                } => {
                    let x = lane_x(lane);
                    (
                        vec![
                            Op::Move(Point::new(x, line_top(self.index, dashed))),
                            Op::Line(Point::new(x, height + EDGE_OVERLAP)),
                        ],
                        color,
                        dashed,
                    )
                }
                Segment::IntoNode {
                    from,
                    color,
                    dashed,
                } => (
                    into_node(lane_x(from), line_top(self.index, dashed), node),
                    color,
                    dashed,
                ),
                Segment::FromNode { to, color, dashed } => {
                    (from_node(node, lane_x(to), height), color, dashed)
                }
            };
            let path = to_path(&ops);
            frame.stroke(&path, line(theme::lane_color(p, color.0), dashed));
        }

        let color = theme::lane_color(p, self.row.color.0);
        let color = if self.dimmed {
            color.scale_alpha(0.3)
        } else {
            color
        };
        match &self.node {
            Node::Wip => {
                let circle = Path::circle(node, NODE_RADIUS - 2.0);
                frame.fill(&circle, p.bg_graph);
                frame.stroke(&circle, line(color, true));
            }
            Node::Merge => frame.fill(&Path::circle(node, MERGE_RADIUS), color),
            Node::Stash => {
                // Dashed square like GitKraken's; the archive icon lies on
                // top (`graph_with_avatar`), M11-05.
                let square = Path::rectangle(
                    Point::new(node.x - STASH_SIZE / 2.0, node.y - STASH_SIZE / 2.0),
                    iced::Size::new(STASH_SIZE, STASH_SIZE),
                );
                frame.fill(&square, p.bg_graph);
                frame.stroke(&square, line(color, true));
            }
            Node::Commit {
                avatar: Some(_), ..
            } => {
                // The avatar itself is an image on top (`graph_with_avatar`);
                // a ring in the lane color frames it.
                frame.stroke(&Path::circle(node, ring_radius(p)), line(color, false));
            }
            Node::Commit { initials, .. } => {
                frame.fill(&Path::circle(node, NODE_RADIUS), color);
                if p.node_style == theme::NodeStyle::Ringed {
                    frame.stroke(&Path::circle(node, ring_radius(p)), line(color, false));
                }
                // Canvas text escapes the cell's clip: a node cut off by a
                // narrow Graph column (M11-02) shows no initials.
                if node.x + NODE_RADIUS > bounds.width {
                    return vec![frame.into_geometry()];
                }
                frame.fill_text(canvas::Text {
                    content: initials.clone(),
                    position: node,
                    color: p.node_text,
                    size: 8.into(),
                    align_x: iced::widget::text::Alignment::Center,
                    align_y: alignment::Vertical::Center,
                    ..canvas::Text::default()
                });
            }
        }

        vec![frame.into_geometry()]
    }
}

/// Local y where a line entering row `index` from above starts: past the
/// top edge by at least [`EDGE_OVERLAP`]. A dashed line starts where the
/// pattern of the whole graph (from the top of row 0) is at its beginning,
/// so the dashes continue across rows.
fn line_top(index: usize, dashed: bool) -> f32 {
    if !dashed {
        return -EDGE_OVERLAP;
    }
    let period = f64::from(DASH[0] + DASH[1]);
    let phase = (index as f64 * f64::from(ROW_HEIGHT)) % period;
    (-phase - period) as f32
}

/// One step of an edge path (kept as data so the geometry is testable).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Move(Point),
    Line(Point),
    /// Rounded corner: quadratic curve through the corner point.
    Corner {
        corner: Point,
        to: Point,
    },
}

fn to_path(ops: &[Op]) -> Path {
    Path::new(|builder| {
        for op in ops {
            match *op {
                Op::Move(point) => builder.move_to(point),
                Op::Line(point) => builder.line_to(point),
                Op::Corner { corner, to } => builder.quadratic_curve_to(corner, to),
            }
        }
    })
}

/// Edge from above in the lane at `x` into `node`: straight down from
/// `top` (past the row edge); in another lane it turns 90° with a rounded
/// corner at the node's height and runs straight into the node.
fn into_node(x: f32, top: f32, node: Point) -> Vec<Op> {
    let mut ops = vec![Op::Move(Point::new(x, top))];
    if (x - node.x).abs() < f32::EPSILON {
        ops.push(Op::Line(node));
        return ops;
    }
    let toward = (node.x - x).signum();
    let radius = corner_radius(x, node);
    ops.push(Op::Line(Point::new(x, node.y - radius)));
    ops.push(Op::Corner {
        corner: Point::new(x, node.y),
        to: Point::new(x + toward * radius, node.y),
    });
    ops.push(Op::Line(node));
    ops
}

/// Edge from `node` to the lane at `x` and down past the bottom edge of a
/// row `height` high: straight in the node's lane, else straight out of the
/// node to the lane and a rounded 90° turn down.
fn from_node(node: Point, x: f32, height: f32) -> Vec<Op> {
    let bottom = Point::new(x, height + EDGE_OVERLAP);
    let mut ops = vec![Op::Move(node)];
    if (x - node.x).abs() < f32::EPSILON {
        ops.push(Op::Line(bottom));
        return ops;
    }
    let toward = (x - node.x).signum();
    let radius = corner_radius(x, node);
    ops.push(Op::Line(Point::new(x - toward * radius, node.y)));
    ops.push(Op::Corner {
        corner: Point::new(x, node.y),
        to: Point::new(x, node.y + radius),
    });
    ops.push(Op::Line(bottom));
    ops
}

/// [`CORNER_RADIUS`], smaller when the lanes are closer than that.
fn corner_radius(x: f32, node: Point) -> f32 {
    CORNER_RADIUS.min((x - node.x).abs())
}

fn line(color: Color, dashed: bool) -> Stroke<'static> {
    let stroke = Stroke::default().with_color(color).with_width(LINE_WIDTH);
    if dashed {
        Stroke {
            line_dash: LineDash {
                segments: &DASH,
                offset: 0,
            },
            ..stroke
        }
    } else {
        stroke
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(name: &str, kind: RefKind, is_head: bool) -> Reference {
        Reference {
            name: name.into(),
            kind,
            target: "c".into(),
            is_head,
            upstream: None,
            ahead_behind: None,
        }
    }

    fn commit() -> Commit {
        Commit {
            id: "c".into(),
            parents: Vec::new(),
            summary: String::new(),
            body: String::new(),
            author: String::new(),
            email: String::new(),
            author_time: 0,
            commit_time: 0,
        }
    }

    fn texts(labels: &[RefLabel]) -> Vec<&str> {
        labels.iter().map(|l| l.text.as_str()).collect()
    }

    fn on_branch() -> Head {
        Head::Branch {
            name: "main".into(),
            target: "c".into(),
        }
    }

    #[test]
    fn local_and_remote_with_same_name_share_a_pill() {
        let refs = [
            reference("origin/main", RefKind::RemoteBranch, false),
            reference("feature", RefKind::LocalBranch, false),
            reference("main", RefKind::LocalBranch, true),
            reference("upstream/main", RefKind::RemoteBranch, false),
            reference("origin/other", RefKind::RemoteBranch, false),
            reference("v1.0", RefKind::Tag, false),
        ];
        let refs: Vec<&Reference> = refs.iter().collect();
        let labels = ref_labels(&refs, &on_branch(), &commit());
        assert_eq!(texts(&labels), ["main", "feature", "origin/other", "v1.0"]);
        assert_eq!(labels[0].remotes, ["origin", "upstream"]);
        assert_eq!(labels[0].description(), "main (origin, upstream)");
        assert_eq!(labels[1].description(), "feature");
        assert!(labels[0].head);
        assert_eq!(labels[3].kind, LabelKind::Tag);
    }

    #[test]
    fn detached_head_marker_comes_first() {
        let refs = [reference("v2", RefKind::Tag, false)];
        let refs: Vec<&Reference> = refs.iter().collect();
        let head = Head::Detached { target: "c".into() };
        let labels = ref_labels(&refs, &head, &commit());
        assert_eq!(texts(&labels), ["HEAD", "v2"]);
        assert_eq!(labels[0].kind, LabelKind::DetachedHead);
    }

    #[test]
    fn no_refs_no_labels() {
        assert!(ref_labels(&[], &on_branch(), &commit()).is_empty());
    }

    /// The renderer translates each row's mesh to the physical pixel grid
    /// (top left rounded) and clips it to its snapped bounds. A line from
    /// local `top` to `bottom` must cover that clip, plus half a pixel for
    /// anti-aliasing, at every row and scale (M6-23).
    fn covers_snapped_row(top: f32, bottom: f32, scale: f32, scroll: f32, row: usize) -> bool {
        let row_top = (row as f32 * ROW_HEIGHT - scroll) * scale;
        let row_bottom = row_top + ROW_HEIGHT * scale;
        let (clip_top, clip_bottom) = (row_top.round(), row_bottom.round());
        let line_top = clip_top + top * scale;
        let line_bottom = clip_top + bottom * scale;
        line_top <= clip_top - 0.5 && line_bottom >= clip_bottom + 0.5
    }

    #[test]
    fn lines_join_across_rows_at_fractional_scales() {
        // System scale × zoom: 1.25 × 110 % and others.
        let scales = [1.0, 1.1, 1.25, 1.375, 1.5, 1.65, 1.75, 2.25, 0.9];
        let height = ROW_HEIGHT;
        let mut naive_gap = false;
        for scale in scales {
            for scroll in [0.0, 7.3, 1234.56] {
                for row in 0..400 {
                    for dashed in [false, true] {
                        // Through segments and the straight lead-in of a
                        // segment entering a node.
                        let top = line_top(row, dashed);
                        assert!(
                            covers_snapped_row(top, height + EDGE_OVERLAP, scale, scroll, row),
                            "gap at scale {scale}, scroll {scroll}, row {row}"
                        );
                    }
                    naive_gap |= !covers_snapped_row(0.0, height, scale, scroll, row);
                }
            }
        }
        assert!(naive_gap, "lines ending at the row edges leave gaps");
    }

    #[test]
    fn edges_turn_with_rounded_corners() {
        let node = Point::new(lane_x(0), ROW_HEIGHT / 2.0);
        let x = lane_x(2);
        // Joining a node from another lane: down, corner, straight in.
        let ops = into_node(x, -EDGE_OVERLAP, node);
        assert_eq!(
            ops,
            [
                Op::Move(Point::new(x, -EDGE_OVERLAP)),
                Op::Line(Point::new(x, node.y - CORNER_RADIUS)),
                Op::Corner {
                    corner: Point::new(x, node.y),
                    to: Point::new(x - CORNER_RADIUS, node.y),
                },
                Op::Line(node),
            ]
        );
        // Leaving a node to another lane: straight out, corner, down past
        // the bottom edge (M6-23).
        let ops = from_node(node, x, ROW_HEIGHT);
        assert_eq!(
            ops,
            [
                Op::Move(node),
                Op::Line(Point::new(x - CORNER_RADIUS, node.y)),
                Op::Corner {
                    corner: Point::new(x, node.y),
                    to: Point::new(x, node.y + CORNER_RADIUS),
                },
                Op::Line(Point::new(x, ROW_HEIGHT + EDGE_OVERLAP)),
            ]
        );
        // To the left the corner mirrors.
        let right = Point::new(lane_x(3), node.y);
        assert!(matches!(
            from_node(right, lane_x(1), ROW_HEIGHT)[1],
            Op::Line(p) if p.x == lane_x(1) + CORNER_RADIUS
        ));
        // The same lane stays straight.
        assert_eq!(
            from_node(node, node.x, ROW_HEIGHT),
            [
                Op::Move(node),
                Op::Line(Point::new(node.x, ROW_HEIGHT + EDGE_OVERLAP))
            ]
        );
        assert_eq!(into_node(node.x, -9.0, node).len(), 2);
        // The radius fits between lanes and above the node.
        const { assert!(CORNER_RADIUS <= LANE_WIDTH && CORNER_RADIUS < ROW_HEIGHT / 2.0) };
        // Thinner than the 2 px before M7-18.
        const { assert!(LINE_WIDTH < 2.0) };
    }

    #[test]
    fn dashes_continue_across_rows() {
        let period = DASH[0] + DASH[1];
        for row in [0usize, 1, 2, 6, 7, 99, 100_000] {
            let top = line_top(row, true);
            assert!(top <= -EDGE_OVERLAP);
            // The pattern starts at a multiple of the period of the graph.
            let start = row as f64 * f64::from(ROW_HEIGHT) + f64::from(top);
            let rest = start.rem_euclid(f64::from(period));
            assert!(
                rest < 1e-3 || rest > f64::from(period) - 1e-3,
                "row {row}: {rest}"
            );
        }
        assert_eq!(line_top(5, false), -EDGE_OVERLAP);
    }

    #[test]
    fn row_backgrounds_snap_to_pixels() {
        let theme = crate::theme::app_theme(crate::theme::ThemeKind::Dark);
        for selected in [false, true] {
            let style =
                crate::theme::graph_row(selected, 0)(&theme, iced::widget::button::Status::Hovered);
            assert!(style.snap);
        }
    }

    #[test]
    fn visible_range_with_overscan() {
        // 100 px viewport at the top: rows 0..4 visible, plus overscan below.
        assert_eq!(visible_range(0.0, 100.0, 1000), (0, 4 + OVERSCAN));
        // Scrolled to row 50.
        let (first, last) = visible_range(50.0 * ROW_HEIGHT, 300.0, 1000);
        assert_eq!((first, last), (50 - OVERSCAN, 60 + OVERSCAN));
        // Clamped at the end and for tiny lists.
        // Scrolled to the very end: 1000 rows, 600 px viewport.
        assert_eq!(visible_range(29_400.0, 600.0, 1000), (970, 1000));
        assert_eq!(visible_range(0.0, 600.0, 3), (0, 3));
        assert_eq!(visible_range(0.0, 600.0, 0), (0, 0));
        // Offset beyond the content (list shrank after a refresh).
        assert_eq!(visible_range(1e6, 600.0, 5), (5, 5));
    }

    #[test]
    fn reveal_scrolls_only_when_needed() {
        let height = 10.0 * ROW_HEIGHT;
        assert_eq!(reveal_offset(5, 0.0, height), None);
        assert_eq!(reveal_offset(9, 0.0, height), None);
        assert_eq!(reveal_offset(10, 0.0, height), Some(ROW_HEIGHT));
        assert_eq!(reveal_offset(100, 0.0, height), Some(91.0 * ROW_HEIGHT));
        assert_eq!(
            reveal_offset(3, 50.0 * ROW_HEIGHT, height),
            Some(3.0 * ROW_HEIGHT)
        );
    }

    #[test]
    fn shown_columns_follow_the_settings() {
        let mut appearance = crate::settings::Appearance::default();
        let w = ColumnWidths::default();
        let all = Columns::shown(&appearance);
        assert_eq!(
            all,
            Columns {
                author: true,
                date: true,
                sha: true
            }
        );
        assert_eq!(all.sum(&w, 48.0), w.refs + 48.0 + w.author + w.date + w.sha);
        appearance.graph_date = false;
        let some = Columns::shown(&appearance);
        assert!(some.author && !some.date && some.sha);
        assert_eq!(some.sum(&w, 48.0), w.refs + 48.0 + w.author + w.sha);
    }

    #[test]
    fn graph_width_is_clamped() {
        assert_eq!(graph_width(0), graph_width(2));
        assert!(graph_width(5) > graph_width(2));
        assert_eq!(graph_width(40), graph_width(12));
    }
}
