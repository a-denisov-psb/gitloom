//! Colors and widget styles. All visual constants live here so a later
//! theme system only has to touch this module.

use git_core::{ChangeKind, RefKind};
use iced::theme::Palette;
use iced::widget::{button, container, pane_grid};
use iced::{Background, Border, Color, Theme};

const fn hex(value: u32) -> Color {
    Color::from_rgb8(
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    )
}

pub const BG_GRAPH: Color = hex(0x1f232b);
pub const BG_PANEL: Color = hex(0x252a33);
pub const BG_CHROME: Color = hex(0x181b21);
pub const BG_HOVER: Color = hex(0x2d333e);
pub const BG_SELECTED: Color = hex(0x2e4058);
pub const BORDER: Color = hex(0x343a46);
pub const TEXT: Color = hex(0xd5d9e0);
pub const TEXT_MUTED: Color = hex(0x8a93a3);
pub const ACCENT: Color = hex(0x4f9dde);
pub const SUCCESS: Color = hex(0x5fb878);
pub const WARNING: Color = hex(0xe5c07b);
pub const DANGER: Color = hex(0xe06c75);

/// Colors cycled through by graph lanes.
pub const LANE_COLORS: [Color; 8] = [
    hex(0x4f9dde),
    hex(0xe06c75),
    hex(0x5fb878),
    hex(0xe5c07b),
    hex(0xc678dd),
    hex(0x56b6c2),
    hex(0xd19a66),
    hex(0x98c379),
];

pub fn app_theme() -> Theme {
    Theme::custom(
        "Gitloom Dark",
        Palette {
            background: BG_GRAPH,
            text: TEXT,
            primary: ACCENT,
            success: SUCCESS,
            warning: WARNING,
            danger: DANGER,
        },
    )
}

fn filled(color: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(color)),
        ..container::Style::default()
    }
}

/// Root background; visible through 1px gaps it acts as divider lines.
pub fn root(_: &Theme) -> container::Style {
    filled(BORDER)
}

/// Tab bar, toolbar and status bar.
pub fn chrome(_: &Theme) -> container::Style {
    filled(BG_CHROME)
}

pub fn panel(_: &Theme) -> container::Style {
    filled(BG_PANEL)
}

pub fn graph(_: &Theme) -> container::Style {
    filled(BG_GRAPH)
}

pub fn column_header(_: &Theme) -> container::Style {
    filled(BG_PANEL)
}

pub fn panes(_: &Theme) -> pane_grid::Style {
    let highlight = pane_grid::Highlight {
        background: Background::Color(ACCENT.scale_alpha(0.2)),
        border: Border::default(),
    };
    pane_grid::Style {
        hovered_region: highlight,
        picked_split: pane_grid::Line {
            color: ACCENT,
            width: 2.0,
        },
        hovered_split: pane_grid::Line {
            color: ACCENT,
            width: 2.0,
        },
    }
}

fn button_style(background: Option<Color>, text_color: Color) -> button::Style {
    button::Style {
        background: background.map(Background::Color),
        text_color,
        border: Border::default().rounded(3),
        ..button::Style::default()
    }
}

/// Rows in lists (graph, sidebar, file lists).
pub fn list_item(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let background = match (selected, status) {
            (true, _) => Some(BG_SELECTED),
            (false, button::Status::Hovered | button::Status::Pressed) => Some(BG_HOVER),
            _ => None,
        };
        button::Style {
            border: Border::default(),
            ..button_style(background, TEXT)
        }
    }
}

pub fn tab(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let (background, text_color) = match (selected, status) {
            (true, _) => (Some(BG_GRAPH), TEXT),
            (false, button::Status::Hovered) => (Some(BG_HOVER), TEXT),
            _ => (None, TEXT_MUTED),
        };
        button::Style {
            border: Border::default(),
            ..button_style(background, text_color)
        }
    }
}

/// Flat button that only shows a background on hover.
pub fn flat(_: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Hovered | button::Status::Pressed => button_style(Some(BG_HOVER), TEXT),
        button::Status::Disabled => button_style(None, TEXT_MUTED.scale_alpha(0.5)),
        button::Status::Active => button_style(None, TEXT),
    }
}

pub fn section_header(_: &Theme, status: button::Status) -> button::Style {
    let text_color = match status {
        button::Status::Hovered => TEXT,
        _ => TEXT_MUTED,
    };
    button::Style {
        border: Border::default(),
        ..button_style(None, text_color)
    }
}

pub fn ref_pill(kind: RefKind, is_head: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        let color = match kind {
            RefKind::LocalBranch if is_head => ACCENT,
            RefKind::LocalBranch => LANE_COLORS[5],
            RefKind::RemoteBranch => TEXT_MUTED,
            RefKind::Tag => WARNING,
        };
        container::Style {
            background: Some(Background::Color(color.scale_alpha(0.25))),
            border: Border::default().rounded(3).width(1).color(color),
            text_color: Some(TEXT),
            ..container::Style::default()
        }
    }
}

pub fn change_color(kind: ChangeKind) -> Color {
    match kind {
        ChangeKind::Added | ChangeKind::Untracked => SUCCESS,
        ChangeKind::Modified | ChangeKind::Renamed => WARNING,
        ChangeKind::Deleted | ChangeKind::Conflicted => DANGER,
    }
}

/// Palette color for a lane color id from `git-graph`.
pub fn lane_color(color: u32) -> Color {
    LANE_COLORS[color as usize % LANE_COLORS.len()]
}

/// Graph table row; the selected row is tinted with its lane color.
pub fn graph_row(selected: bool, lane: Color) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let background = match (selected, status) {
            (true, _) => Some(Color::from_rgb(
                BG_GRAPH.r * 0.75 + lane.r * 0.25,
                BG_GRAPH.g * 0.75 + lane.g * 0.25,
                BG_GRAPH.b * 0.75 + lane.b * 0.25,
            )),
            (false, button::Status::Hovered | button::Status::Pressed) => Some(BG_HOVER),
            _ => None,
        };
        button::Style {
            border: Border::default(),
            ..button_style(background, TEXT)
        }
    }
}

/// In-tab error banner.
pub fn error_banner(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(DANGER.scale_alpha(0.18))),
        border: Border::default().width(1).color(DANGER.scale_alpha(0.6)),
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

pub fn tooltip(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_CHROME)),
        border: Border::default().rounded(3).width(1).color(BORDER),
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

/// Hunk header row in the diff view.
pub fn diff_hunk_header(_: &Theme) -> container::Style {
    filled(BG_PANEL)
}

/// Diff line background by kind; selected lines are stronger and outlined.
pub fn diff_line(
    kind: git_core::DiffLineKind,
    selected: bool,
) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        let alpha = if selected { 0.38 } else { 0.16 };
        let style = match kind {
            git_core::DiffLineKind::Context => container::Style::default(),
            git_core::DiffLineKind::Added => filled(SUCCESS.scale_alpha(alpha)),
            git_core::DiffLineKind::Removed => filled(DANGER.scale_alpha(alpha)),
        };
        if selected {
            container::Style {
                border: Border::default().width(1).color(ACCENT.scale_alpha(0.8)),
                ..style
            }
        } else {
            style
        }
    }
}

/// Conflict line in the ours (blue) or theirs (orange) panel; picked lines
/// are stronger.
pub fn conflict_line(ours: bool, picked: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        let color = if ours { ACCENT } else { WARNING };
        filled(color.scale_alpha(if picked { 0.32 } else { 0.12 }))
    }
}

/// Selected segment of a segmented control (diff modes).
pub fn mode_active(_: &Theme, _: button::Status) -> button::Style {
    button_style(Some(BG_SELECTED), TEXT)
}

/// Toolbar pickers: readable text even while they cannot be opened yet.
pub fn picker(_: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Hovered | button::Status::Pressed => button_style(Some(BG_HOVER), TEXT),
        _ => button_style(None, TEXT),
    }
}

/// App-wide informational banner.
pub fn notice_banner(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(WARNING.scale_alpha(0.16))),
        border: Border::default().width(1).color(WARNING.scale_alpha(0.5)),
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

/// Operation in progress (merge, rebase, ...) at the top of a tab.
pub fn state_banner(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(ACCENT.scale_alpha(0.16))),
        border: Border::default().width(1).color(ACCENT.scale_alpha(0.5)),
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

/// Inline text input (e.g. new branch name); red border on an error.
pub fn inline_input(
    error: bool,
) -> impl Fn(&Theme, iced::widget::text_input::Status) -> iced::widget::text_input::Style {
    move |theme, status| {
        let mut style = iced::widget::text_input::default(theme, status);
        if error {
            style.border = style.border.color(DANGER).width(1);
        }
        style
    }
}

/// Small dropdown arrow next to a toolbar button.
pub fn dropdown_arrow(_: &Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_MUTED),
        border: Border::default().rounded(3),
        ..container::Style::default()
    }
}

/// Dimmed backdrop behind a modal dialog.
pub fn backdrop(_: &Theme) -> container::Style {
    filled(Color::BLACK.scale_alpha(0.55))
}

pub fn dialog(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_PANEL)),
        border: Border::default().rounded(6).width(1).color(BORDER),
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

pub fn dialog_details(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_CHROME)),
        border: Border::default().rounded(3).width(1).color(BORDER),
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

pub fn menu(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(BG_PANEL)),
        border: Border::default().rounded(4).width(1).color(BORDER),
        shadow: iced::Shadow {
            color: Color::BLACK.scale_alpha(0.4),
            offset: iced::Vector::new(0.0, 3.0),
            blur_radius: 10.0,
        },
        text_color: Some(TEXT),
        ..container::Style::default()
    }
}

pub fn menu_separator(_: &Theme) -> container::Style {
    filled(BORDER)
}

pub fn menu_item(_: &Theme, status: button::Status) -> button::Style {
    match status {
        button::Status::Hovered | button::Status::Pressed => button_style(Some(BG_SELECTED), TEXT),
        button::Status::Disabled => button_style(None, TEXT_MUTED.scale_alpha(0.6)),
        button::Status::Active => button_style(None, TEXT),
    }
}
