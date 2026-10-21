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

pub fn lane_color(lane: usize) -> Color {
    LANE_COLORS[lane % LANE_COLORS.len()]
}
