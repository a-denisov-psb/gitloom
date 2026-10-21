//! Center pane: commit table with the lane graph computed by `git-graph`.
//!
//! Every row draws its own slice of the graph (segments and node), so rows
//! can be rendered independently.

use git_core::{Commit, Head, RefKind, Reference};
use git_graph::{Row, Segment};
use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, LineDash, Path, Stroke};
use iced::widget::{button, column, container, row, scrollable, text, tooltip, Column};
use iced::{alignment, Color, Element, Fill, Length, Point, Rectangle, Renderer, Theme};

use crate::app::Message;
use crate::format;
use crate::repo_view::{RepoView, Selection};
use crate::theme;
use crate::ui::muted;

pub const ROW_HEIGHT: f32 = 30.0;
const COL_REFS: f32 = 200.0;
const COL_AUTHOR: f32 = 130.0;
const COL_DATE: f32 = 135.0;
const COL_SHA: f32 = 70.0;
const LANE_WIDTH: f32 = 18.0;
const GRAPH_PADDING: f32 = 6.0;
const MIN_LANES: u16 = 2;
const MAX_LANES: u16 = 12;
const NODE_RADIUS: f32 = 9.0;
const MERGE_RADIUS: f32 = 4.5;
const DASH: [f32; 2] = [4.0, 3.0];

pub fn view(repo: &RepoView) -> Element<'_, Message> {
    let graph_width = graph_width(repo.snapshot.layout.max_width);
    let header = container(
        row![
            cell(muted("BRANCH / TAG").size(11), COL_REFS),
            cell(muted("GRAPH").size(11), graph_width),
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

    let rows = (0..repo.snapshot.layout.rows.len()).map(|r| graph_row(repo, r, graph_width));

    container(column![
        header,
        scrollable(Column::with_children(rows)).height(Fill)
    ])
    .width(Fill)
    .height(Fill)
    .style(theme::graph)
    .into()
}

/// Width of the graph column for `lanes` lanes (clamped to 2..=12).
pub fn graph_width(lanes: u16) -> f32 {
    let lanes = lanes.clamp(MIN_LANES, MAX_LANES);
    GRAPH_PADDING * 2.0 + LANE_WIDTH * f32::from(lanes)
}

/// One table row: the WIP row or a commit row.
pub fn graph_row(repo: &RepoView, row_index: usize, graph_width: f32) -> Element<'_, Message> {
    let snapshot = &repo.snapshot;
    let Some(layout_row) = snapshot.layout.rows.get(row_index) else {
        return container(text("")).height(ROW_HEIGHT).into();
    };
    let color = theme::lane_color(layout_row.color.0);

    if snapshot.has_wip && row_index == 0 {
        let tree = &snapshot.working_tree;
        let summary = row![
            muted("// WIP"),
            muted(format!("✎ {}", tree.unstaged.len())),
            muted(format!("✓ {}", tree.staged.len())),
        ]
        .spacing(12);
        let content = row![
            cell(text(""), COL_REFS),
            cell(graph_cell(layout_row, Node::Wip), graph_width),
            cell(summary, Fill),
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
    let node = if commit.parents.len() > 1 {
        Node::Merge
    } else {
        Node::Commit(format::initials(&commit.author))
    };
    let summary = if commit.parents.len() > 1 {
        muted(&commit.summary)
    } else {
        text(&commit.summary)
    };

    let refs: Vec<&Reference> = snapshot.refs_at(index).collect();
    let content = row![
        cell(
            ref_pills(ref_labels(&refs, &snapshot.head, commit)),
            COL_REFS
        ),
        cell(graph_cell(layout_row, node), graph_width),
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
        color,
        Message::CommitSelected(commit.id.clone()),
    )
}

/// What a reference pill shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefLabel {
    pub text: String,
    pub kind: LabelKind,
    /// The checked-out branch (or the detached HEAD marker).
    pub head: bool,
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
                kind: LabelKind::DetachedHead,
                head: true,
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
        let check = if local.is_head { "✓ " } else { "" };
        let text = if remotes.is_empty() {
            format!("{check}{}", local.name)
        } else {
            format!("{check}{} · {}", local.name, remotes.join(", "))
        };
        labels.push(RefLabel {
            text,
            kind: LabelKind::Local,
            head: local.is_head,
        });
    }
    for kind in [RefKind::RemoteBranch, RefKind::Tag] {
        for (j, reference) in refs.iter().enumerate() {
            if !used[j] && reference.kind == kind {
                used[j] = true;
                labels.push(RefLabel {
                    text: reference.name.clone(),
                    kind: if kind == RefKind::Tag {
                        LabelKind::Tag
                    } else {
                        LabelKind::Remote
                    },
                    head: false,
                });
            }
        }
    }
    labels
}

/// First label as pill; the rest collapse into "+N" with a tooltip listing them.
fn ref_pills<'a>(labels: Vec<RefLabel>) -> Element<'a, Message> {
    let Some(first) = labels.first() else {
        return text("").into();
    };
    let mut pills = row![pill(first)].spacing(4).align_y(iced::Center);
    if labels.len() > 1 {
        let rest = Column::with_children(
            labels[1..]
                .iter()
                .map(|label| text(label.text.clone()).size(12).into()),
        )
        .spacing(2);
        pills = pills.push(tooltip(
            muted(format!("+{}", labels.len() - 1)).size(11),
            container(rest).padding([4, 8]).style(theme::tooltip),
            tooltip::Position::Bottom,
        ));
    }
    pills.into()
}

fn pill<'a>(label: &RefLabel) -> Element<'a, Message> {
    let kind = match label.kind {
        LabelKind::Local => RefKind::LocalBranch,
        LabelKind::Remote => RefKind::RemoteBranch,
        LabelKind::Tag => RefKind::Tag,
        LabelKind::DetachedHead => RefKind::LocalBranch,
    };
    container(
        text(label.text.clone())
            .size(12)
            .wrapping(text::Wrapping::None),
    )
    .padding([1, 6])
    .clip(true)
    .style(theme::ref_pill(kind, label.head))
    .into()
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
    lane_color: Color,
    on_press: Message,
) -> Element<'a, Message> {
    button(content.padding([0, 8]).height(Fill))
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
    /// Regular commit with the author's initials.
    Commit(String),
}

/// Graph slice of one row.
struct GraphCell {
    row: Row,
    node: Node,
}

fn graph_cell<'a>(row: &Row, node: Node) -> Element<'a, Message> {
    canvas::Canvas::new(GraphCell {
        row: row.clone(),
        node,
    })
    .width(Fill)
    .height(Fill)
    .into()
}

fn lane_x(lane: u16) -> f32 {
    GRAPH_PADDING + LANE_WIDTH * f32::from(lane) + LANE_WIDTH / 2.0
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
        let height = bounds.height;
        let mid = height / 2.0;
        let node = Point::new(lane_x(self.row.lane), mid);

        for segment in &self.row.segments {
            let (path, color, dashed) = match *segment {
                Segment::Through {
                    lane,
                    color,
                    dashed,
                } => (
                    Path::line(
                        Point::new(lane_x(lane), 0.0),
                        Point::new(lane_x(lane), height),
                    ),
                    color,
                    dashed,
                ),
                Segment::IntoNode {
                    from,
                    color,
                    dashed,
                } => (curve(Point::new(lane_x(from), 0.0), node), color, dashed),
                Segment::FromNode { to, color, dashed } => {
                    (curve(node, Point::new(lane_x(to), height)), color, dashed)
                }
            };
            frame.stroke(&path, line(theme::lane_color(color.0), dashed));
        }

        let color = theme::lane_color(self.row.color.0);
        match &self.node {
            Node::Wip => {
                let circle = Path::circle(node, NODE_RADIUS - 2.0);
                frame.fill(&circle, theme::BG_GRAPH);
                frame.stroke(&circle, line(color, true));
            }
            Node::Merge => frame.fill(&Path::circle(node, MERGE_RADIUS), color),
            Node::Commit(initials) => {
                frame.fill(&Path::circle(node, NODE_RADIUS), color);
                frame.fill_text(canvas::Text {
                    content: initials.clone(),
                    position: node,
                    color: Color::WHITE,
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

/// Straight line for vertical connections, smooth S-curve for lane changes.
fn curve(from: Point, to: Point) -> Path {
    if (from.x - to.x).abs() < f32::EPSILON {
        return Path::line(from, to);
    }
    let bend = (to.y - from.y) / 2.0;
    Path::new(|builder| {
        builder.move_to(from);
        builder.bezier_curve_to(
            Point::new(from.x, from.y + bend),
            Point::new(to.x, to.y - bend),
            to,
        );
    })
}

fn line(color: Color, dashed: bool) -> Stroke<'static> {
    let stroke = Stroke::default().with_color(color).with_width(2.0);
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
        assert_eq!(
            texts(&labels),
            [
                "✓ main · origin, upstream",
                "feature",
                "origin/other",
                "v1.0"
            ]
        );
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

    #[test]
    fn graph_width_is_clamped() {
        assert_eq!(graph_width(0), graph_width(2));
        assert!(graph_width(5) > graph_width(2));
        assert_eq!(graph_width(40), graph_width(12));
    }
}
