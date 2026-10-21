//! Center pane: commit graph with WIP row on top.
//!
//! Every row draws its own graph segment (line up, line down, node), so the
//! graph scales with a virtualized list later. Lane assignment is still a
//! placeholder: everything sits in lane 0.

use git_core::{Commit, RefKind, Reference};
use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{alignment, Color, Element, Fill, Length, Point, Rectangle, Renderer, Theme};

use crate::app::Message;
use crate::format;
use crate::repo_view::{RepoView, Selection};
use crate::theme;
use crate::ui::muted;

const ROW_HEIGHT: f32 = 30.0;
const COL_REFS: f32 = 200.0;
const COL_GRAPH: f32 = 90.0;
const COL_AUTHOR: f32 = 130.0;
const COL_DATE: f32 = 135.0;
const COL_SHA: f32 = 70.0;
const LANE_WIDTH: f32 = 22.0;
const NODE_RADIUS: f32 = 10.0;

pub fn view(repo: &RepoView) -> Element<'_, Message> {
    let header = container(
        row![
            cell(muted("BRANCH / TAG").size(11), COL_REFS),
            cell(muted("GRAPH").size(11), COL_GRAPH),
            cell(muted("COMMIT MESSAGE").size(11), Fill),
            cell(muted("AUTHOR").size(11), COL_AUTHOR),
            cell(muted("DATE").size(11), COL_DATE),
            cell(muted("SHA").size(11), COL_SHA),
        ]
        .padding([0, 8])
        .align_y(iced::Center),
    )
    .height(24)
    .align_y(alignment::Vertical::Center)
    .style(theme::column_header);

    let has_wip = repo.snapshot.has_wip;
    let wip = has_wip.then(|| wip_row(repo));
    let commits = repo
        .snapshot
        .commits
        .iter()
        .enumerate()
        .map(|(index, commit)| commit_row(repo, index, commit, has_wip || index > 0));

    let rows = Column::with_children(wip.into_iter().chain(commits));

    container(column![header, scrollable(rows).height(Fill)])
        .width(Fill)
        .height(Fill)
        .style(theme::graph)
        .into()
}

fn wip_row(repo: &RepoView) -> Element<'_, Message> {
    let tree = &repo.snapshot.working_tree;
    let summary = row![
        muted("// WIP"),
        muted(format!("✎ {}", tree.unstaged.len())),
        muted(format!("✓ {}", tree.staged.len())),
    ]
    .spacing(12);

    let content = row![
        cell(text(""), COL_REFS),
        cell(
            graph_cell(Node::Wip, 0, false, true, String::new()),
            COL_GRAPH
        ),
        cell(summary, Fill),
    ];

    row_button(
        content,
        repo.selection == Some(Selection::Wip),
        Message::WipSelected,
    )
}

fn commit_row<'a>(
    repo: &'a RepoView,
    index: usize,
    commit: &'a Commit,
    line_up: bool,
) -> Element<'a, Message> {
    let node = if commit.is_merge() {
        Node::Merge
    } else {
        Node::Commit
    };
    let line_down = !commit.parents.is_empty();

    let summary = if commit.is_merge() {
        muted(&commit.summary)
    } else {
        text(&commit.summary)
    };

    let content = row![
        cell(ref_labels(repo, index), COL_REFS),
        cell(
            graph_cell(
                node,
                repo.snapshot
                    .layout
                    .rows
                    .get(repo.snapshot.row_of(index))
                    .map_or(0, |row| usize::from(row.lane)),
                line_up,
                line_down,
                format::initials(&commit.author)
            ),
            COL_GRAPH
        ),
        cell(summary.wrapping(text::Wrapping::None), Fill),
        cell(
            muted(&commit.author).wrapping(text::Wrapping::None),
            COL_AUTHOR
        ),
        cell(
            muted(format::date_time(commit.author_time)).wrapping(text::Wrapping::None),
            COL_DATE
        ),
        cell(
            muted(&commit.id[..commit.id.len().min(7)]).font(iced::Font::MONOSPACE),
            COL_SHA
        ),
    ];

    row_button(
        content,
        repo.selected_commit() == Some(commit.id.as_str()),
        Message::CommitSelected(commit.id.clone()),
    )
}

/// First reference as pill, the rest collapsed into "+N".
fn ref_labels(repo: &RepoView, index: usize) -> Element<'_, Message> {
    let mut refs: Vec<&Reference> = repo.snapshot.refs_at(index).collect();
    refs.sort_by_key(|r| {
        (
            !r.is_head,
            r.kind != RefKind::LocalBranch,
            r.kind == RefKind::Tag,
        )
    });

    let Some(first) = refs.first() else {
        return text("").into();
    };

    let marker = if first.is_head { "✓ " } else { "" };
    let pill = container(
        text(format!("{marker}{}", first.name))
            .size(12)
            .wrapping(text::Wrapping::None),
    )
    .padding([1, 6])
    .clip(true)
    .style(theme::ref_pill(first.kind, first.is_head));

    let mut labels = row![pill].spacing(4).align_y(iced::Center);
    if refs.len() > 1 {
        labels = labels.push(muted(format!("+{}", refs.len() - 1)).size(11));
    }
    labels.into()
}

fn cell<'a>(
    content: impl Into<Element<'a, Message>>,
    width: impl Into<Length>,
) -> Element<'a, Message> {
    container(content)
        .width(width)
        .height(Fill)
        .padding([0, 4])
        .align_y(alignment::Vertical::Center)
        .clip(true)
        .into()
}

fn row_button<'a>(
    content: iced::widget::Row<'a, Message>,
    selected: bool,
    on_press: Message,
) -> Element<'a, Message> {
    button(content.padding([0, 8]).height(Fill))
        .on_press(on_press)
        .padding(0)
        .width(Fill)
        .height(ROW_HEIGHT)
        .style(theme::list_item(selected))
        .into()
}

#[derive(Debug, Clone, Copy)]
enum Node {
    Wip,
    Commit,
    Merge,
}

/// Graph segment for one row.
struct GraphCell {
    node: Node,
    line_up: bool,
    line_down: bool,
    lane: usize,
    initials: String,
}

fn graph_cell<'a>(
    node: Node,
    lane: usize,
    line_up: bool,
    line_down: bool,
    initials: String,
) -> Element<'a, Message> {
    canvas::Canvas::new(GraphCell {
        node,
        line_up,
        line_down,
        lane,
        initials,
    })
    .width(Fill)
    .height(Fill)
    .into()
}

impl canvas::Program<Message> for GraphCell {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let color = theme::lane_color(self.lane);
        let center = Point::new(
            LANE_WIDTH * self.lane as f32 + LANE_WIDTH / 2.0 + 2.0,
            bounds.height / 2.0,
        );
        let line = Stroke::default().with_color(color).with_width(2.0);

        if self.line_up {
            frame.stroke(&Path::line(Point::new(center.x, 0.0), center), line);
        }
        if self.line_down {
            frame.stroke(
                &Path::line(center, Point::new(center.x, bounds.height)),
                line,
            );
        }

        match self.node {
            Node::Wip => {
                let circle = Path::circle(center, NODE_RADIUS - 2.0);
                frame.fill(&circle, theme::BG_GRAPH);
                frame.stroke(
                    &circle,
                    Stroke::default()
                        .with_color(theme::TEXT_MUTED)
                        .with_width(2.0),
                );
            }
            Node::Merge => {
                frame.fill(&Path::circle(center, 5.0), color);
            }
            Node::Commit => {
                frame.fill(&Path::circle(center, NODE_RADIUS), color);
                frame.fill_text(canvas::Text {
                    content: self.initials.clone(),
                    position: center,
                    color: Color::WHITE,
                    size: 9.into(),
                    align_x: iced::widget::text::Alignment::Center,
                    align_y: alignment::Vertical::Center,
                    ..canvas::Text::default()
                });
            }
        }

        vec![frame.into_geometry()]
    }
}
