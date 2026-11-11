//! Colors and widget styles. All colors come from a [`ThemePalette`] chosen
//! at runtime (ADR-027); style functions take it from the iced [`Theme`]
//! they are called with, so a theme switch applies everywhere at once.

use git_core::{ChangeKind, RefKind};
use iced::theme::Palette;
use iced::widget::{button, container, pane_grid, text};
use iced::{Background, Border, Color, Theme};

const fn hex(value: u32) -> Color {
    Color::from_rgb8(
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    )
}

/// All colors of one theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemePalette {
    pub bg_graph: Color,
    pub bg_panel: Color,
    pub bg_chrome: Color,
    pub bg_hover: Color,
    pub bg_selected: Color,
    pub border: Color,
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    /// Text on a filled graph node (initials).
    pub node_text: Color,
    /// Colors cycled through by graph lanes.
    pub lanes: [Color; 8],
    /// Dimmed backdrop behind dialogs.
    pub backdrop: Color,
    /// Style tokens (ADR-048): what differs between themes besides colors.
    /// Graph rows tinted in their lane color at this alpha (0 = off).
    pub row_tint_alpha: f32,
    pub ref_pill: PillStyle,
    /// A line in the lane color from a ref pill to its node.
    pub ref_connector: bool,
    /// Background of the graph's column header.
    pub header_band: Color,
    /// Lines between the graph columns (transparent = none).
    pub column_separator: Color,
    /// Shadow at the edges of the side panels.
    pub panel_shadow: bool,
    pub node_style: NodeStyle,
}

/// How ref pills (branch and tag labels) are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PillStyle {
    /// Outline in the lane color (Dark, Light).
    Outline,
    /// Filled with the lane color, white text (Classic Dark).
    Filled,
}

/// How commit nodes are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStyle {
    /// Avatar or initials circle (today's look).
    Standard,
    /// Classic Dark: the same shapes with a ring in the lane color.
    Ringed,
}

/// The three themes (M11-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    Dark,
    Light,
    ClassicDark,
}

pub const DARK: ThemePalette = ThemePalette {
    bg_graph: hex(0x1f232b),
    bg_panel: hex(0x252a33),
    bg_chrome: hex(0x181b21),
    bg_hover: hex(0x2d333e),
    bg_selected: hex(0x2e4058),
    border: hex(0x343a46),
    text: hex(0xd5d9e0),
    text_muted: hex(0x8a93a3),
    accent: hex(0x4f9dde),
    success: hex(0x5fb878),
    warning: hex(0xe5c07b),
    danger: hex(0xe06c75),
    node_text: Color::WHITE,
    lanes: [
        hex(0x4f9dde),
        hex(0xe06c75),
        hex(0x5fb878),
        hex(0xe5c07b),
        hex(0xc678dd),
        hex(0x56b6c2),
        hex(0xd19a66),
        hex(0x98c379),
    ],
    backdrop: Color::from_rgba(0.0, 0.0, 0.0, 0.55),
    row_tint_alpha: 0.0,
    ref_pill: PillStyle::Outline,
    ref_connector: false,
    header_band: hex(0x252a33),
    column_separator: Color::TRANSPARENT,
    panel_shadow: false,
    node_style: NodeStyle::Standard,
};

pub const LIGHT: ThemePalette = ThemePalette {
    bg_graph: hex(0xffffff),
    bg_panel: hex(0xf4f5f7),
    bg_chrome: hex(0xe8eaee),
    bg_hover: hex(0xe3e7ee),
    bg_selected: hex(0xcfe0f5),
    border: hex(0xd0d4db),
    text: hex(0x1f2328),
    text_muted: hex(0x59616e),
    accent: hex(0x1f6fc5),
    success: hex(0x2e7d32),
    warning: hex(0x875a00),
    danger: hex(0xc62828),
    node_text: Color::WHITE,
    lanes: [
        hex(0x1f6fc5),
        hex(0xd1343f),
        hex(0x2e8b3a),
        hex(0xb7791f),
        hex(0x8e44ad),
        hex(0x0f8b9c),
        hex(0xc0601c),
        hex(0x5c8f1e),
    ],
    backdrop: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
    row_tint_alpha: 0.0,
    ref_pill: PillStyle::Outline,
    ref_connector: false,
    header_band: hex(0xf4f5f7),
    column_separator: Color::TRANSPARENT,
    panel_shadow: false,
    node_style: NodeStyle::Standard,
};

/// "Classic Dark" (M11-01, ADR-048): the look of GitKraken 7.5 with own
/// colors.
pub const CLASSIC_DARK: ThemePalette = ThemePalette {
    bg_graph: hex(0x1e2128),
    bg_panel: hex(0x262a31),
    bg_chrome: hex(0x1b1d22),
    bg_hover: hex(0x2c313a),
    bg_selected: hex(0x1d4a5c),
    border: hex(0x323842),
    text: hex(0xd8dde4),
    text_muted: hex(0x8b94a3),
    accent: hex(0x1aa3c8),
    success: hex(0x2ebf91),
    warning: hex(0xf2a93b),
    danger: hex(0xe04b4b),
    node_text: Color::WHITE,
    lanes: [
        hex(0x1aa3c8),
        hex(0xe0247a),
        hex(0x8b5cc9),
        hex(0x2ebf91),
        hex(0xf2a93b),
        hex(0xe04b4b),
        hex(0x3d7be0),
        hex(0xc9b42c),
    ],
    backdrop: Color::from_rgba(0.0, 0.0, 0.0, 0.55),
    row_tint_alpha: 0.15,
    ref_pill: PillStyle::Filled,
    ref_connector: true,
    header_band: hex(0x2a2f37),
    column_separator: hex(0x323842),
    panel_shadow: true,
    node_style: NodeStyle::Ringed,
};

/// Palette of the theme a style function is called with (the background
/// color tells Classic Dark from Dark without allocating).
pub fn palette(theme: &Theme) -> &'static ThemePalette {
    if theme.palette().background == CLASSIC_DARK.bg_graph {
        &CLASSIC_DARK
    } else if theme.extended_palette().is_dark {
        &DARK
    } else {
        &LIGHT
    }
}

impl ThemeKind {
    pub fn palette(self) -> &'static ThemePalette {
        match self {
            Self::Dark => &DARK,
            Self::Light => &LIGHT,
            Self::ClassicDark => &CLASSIC_DARK,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::ClassicDark => "Classic Dark",
        }
    }
}

/// iced theme for a palette.
pub fn app_theme(kind: ThemeKind) -> Theme {
    let name = format!("{} {}", git_core::identity::DISPLAY_NAME, kind.name());
    let p = kind.palette();
    Theme::custom(
        name,
        Palette {
            background: p.bg_graph,
            text: p.text,
            primary: p.accent,
            success: p.success,
            warning: p.warning,
            danger: p.danger,
        },
    )
}

fn filled(color: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(color)),
        ..container::Style::default()
    }
}

/// Text in a palette color, for `text(..).style(..)`.
pub fn text_with(pick: fn(&ThemePalette) -> Color) -> impl Fn(&Theme) -> text::Style {
    move |theme| text::Style {
        color: Some(pick(palette(theme))),
    }
}

pub fn text_normal(theme: &Theme) -> text::Style {
    text_with(|p| p.text)(theme)
}

pub fn text_muted(theme: &Theme) -> text::Style {
    text_with(|p| p.text_muted)(theme)
}

/// Text of a row dimmed by the commit search.
pub fn text_faint(theme: &Theme) -> text::Style {
    text_with(|p| p.text_muted.scale_alpha(0.55))(theme)
}

pub fn text_accent(theme: &Theme) -> text::Style {
    text_with(|p| p.accent)(theme)
}

pub fn text_success(theme: &Theme) -> text::Style {
    text_with(|p| p.success)(theme)
}

pub fn text_warning(theme: &Theme) -> text::Style {
    text_with(|p| p.warning)(theme)
}

pub fn text_danger(theme: &Theme) -> text::Style {
    text_with(|p| p.danger)(theme)
}

/// Root background; visible through 1px gaps it acts as divider lines.
pub fn root(theme: &Theme) -> container::Style {
    filled(palette(theme).border)
}

/// Thin vertical divider in the toolbar.
pub fn divider(theme: &Theme) -> container::Style {
    filled(palette(theme).border)
}

/// Tab bar, toolbar and status bar.
pub fn chrome(theme: &Theme) -> container::Style {
    filled(palette(theme).bg_chrome)
}

pub fn panel(theme: &Theme) -> container::Style {
    filled(palette(theme).bg_panel)
}

pub fn graph(theme: &Theme) -> container::Style {
    filled(palette(theme).bg_graph)
}

/// Fade at the right edge of a Graph column that clips its lanes (M11-02):
/// from transparent to the row background (graph background, or the
/// selected row's color).
pub fn graph_fade(selected: bool, lane: u32) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let p = palette(theme);
        fade_to(if selected {
            selected_row(p, lane)
        } else {
            p.bg_graph
        })
    }
}

fn fade_to(background: Color) -> container::Style {
    let gradient = iced::gradient::Linear::new(iced::Radians(std::f32::consts::FRAC_PI_2))
        .add_stop(
            0.0,
            Color {
                a: 0.0,
                ..background
            },
        )
        .add_stop(1.0, background);
    container::Style {
        background: Some(Background::Gradient(gradient.into())),
        ..container::Style::default()
    }
}

pub fn selected(theme: &Theme) -> container::Style {
    filled(palette(theme).bg_selected)
}

/// Background of a blame group; neighbouring groups alternate.
pub fn blame_band(odd: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let p = palette(theme);
        if odd {
            filled(p.bg_panel)
        } else {
            filled(p.bg_graph)
        }
    }
}

/// Initials badge in lists (file history).
pub fn avatar(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.accent.scale_alpha(0.25))),
        border: Border::default().rounded(12),
        text_color: Some(p.text),
        ..container::Style::default()
    }
}

pub fn column_header(theme: &Theme) -> container::Style {
    filled(palette(theme).header_band)
}

/// Section without content yet (e.g. submodules): faded text.
pub fn faded(theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(palette(theme).text_muted.scale_alpha(0.6)),
        ..container::Style::default()
    }
}

pub fn panes(theme: &Theme) -> pane_grid::Style {
    let accent = palette(theme).accent;
    let highlight = pane_grid::Highlight {
        background: Background::Color(accent.scale_alpha(0.2)),
        border: Border::default(),
    };
    pane_grid::Style {
        hovered_region: highlight,
        picked_split: pane_grid::Line {
            color: accent,
            width: 2.0,
        },
        hovered_split: pane_grid::Line {
            color: accent,
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
    move |theme, status| {
        let p = palette(theme);
        let background = match (selected, status) {
            (true, _) => Some(p.bg_selected),
            (false, button::Status::Hovered | button::Status::Pressed) => Some(p.bg_hover),
            _ => None,
        };
        // Disabled entries (e.g. repository categories without a
        // repository, M8-01) are muted.
        let text = if status == button::Status::Disabled && !selected {
            p.text_muted
        } else {
            p.text
        };
        button::Style {
            border: Border::default(),
            ..button_style(background, text)
        }
    }
}

pub fn tab(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let p = palette(theme);
        let (background, text_color) = match (selected, status) {
            (true, _) => (Some(p.bg_graph), p.text),
            (false, button::Status::Hovered) => (Some(p.bg_hover), p.text),
            _ => (None, p.text_muted),
        };
        button::Style {
            border: Border::default(),
            ..button_style(background, text_color)
        }
    }
}

/// Flat button that only shows a background on hover.
pub fn flat(theme: &Theme, status: button::Status) -> button::Style {
    let p = palette(theme);
    match status {
        button::Status::Hovered | button::Status::Pressed => button_style(Some(p.bg_hover), p.text),
        button::Status::Disabled => button_style(None, p.text_muted.scale_alpha(0.5)),
        button::Status::Active => button_style(None, p.text),
    }
}

pub fn section_header(theme: &Theme, status: button::Status) -> button::Style {
    let p = palette(theme);
    let text_color = match status {
        button::Status::Hovered => p.text,
        _ => p.text_muted,
    };
    button::Style {
        border: Border::default(),
        ..button_style(None, text_color)
    }
}

pub fn ref_pill(kind: RefKind, is_head: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let p = palette(theme);
        let color = match kind {
            RefKind::LocalBranch if is_head => p.accent,
            RefKind::LocalBranch => p.lanes[5],
            RefKind::RemoteBranch => p.text_muted,
            RefKind::Tag => p.warning,
        };
        container::Style {
            background: Some(Background::Color(color.scale_alpha(0.25))),
            border: Border::default().rounded(3).width(1).color(color),
            text_color: Some(p.text),
            ..container::Style::default()
        }
    }
}

pub fn change_color(p: &ThemePalette, kind: ChangeKind) -> Color {
    match kind {
        ChangeKind::Added | ChangeKind::Untracked => p.success,
        ChangeKind::Modified | ChangeKind::Renamed => p.warning,
        ChangeKind::Deleted | ChangeKind::Conflicted => p.danger,
    }
}

/// Palette color for a lane color id from `git-graph`.
pub fn lane_color(p: &ThemePalette, color: u32) -> Color {
    p.lanes[color as usize % p.lanes.len()]
}

/// Graph table row; the selected row is tinted with its lane color.
/// Background of a selected graph row: the graph background tinted with
/// the row's lane color.
fn selected_row(p: &ThemePalette, lane: u32) -> Color {
    let lane = lane_color(p, lane);
    Color::from_rgb(
        p.bg_graph.r * 0.75 + lane.r * 0.25,
        p.bg_graph.g * 0.75 + lane.g * 0.25,
        p.bg_graph.b * 0.75 + lane.b * 0.25,
    )
}

pub fn graph_row(selected: bool, lane: u32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let p = palette(theme);
        let background = match (selected, status) {
            (true, _) => Some(selected_row(p, lane)),
            (false, button::Status::Hovered | button::Status::Pressed) => Some(p.bg_hover),
            _ => None,
        };
        button::Style {
            border: Border::default(),
            // Snapped to physical pixels: neighbouring rows share their
            // edge instead of blending two anti-aliased edges into a seam
            // at fractional scale factors (M6-23).
            snap: true,
            ..button_style(background, p.text)
        }
    }
}

/// In-tab error banner.
pub fn error_banner(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.danger.scale_alpha(0.18))),
        border: Border::default().width(1).color(p.danger.scale_alpha(0.6)),
        text_color: Some(p.text),
        ..container::Style::default()
    }
}

pub fn tooltip(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.bg_chrome)),
        border: Border::default().rounded(3).width(1).color(p.border),
        text_color: Some(p.text),
        ..container::Style::default()
    }
}

/// Hunk header row in the diff view.
pub fn diff_hunk_header(theme: &Theme) -> container::Style {
    filled(palette(theme).bg_panel)
}

/// Diff line background by kind; selected lines are stronger and outlined.
pub fn diff_line(
    kind: git_core::DiffLineKind,
    selected: bool,
) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let p = palette(theme);
        let alpha = if selected { 0.38 } else { 0.16 };
        let style = match kind {
            git_core::DiffLineKind::Context => container::Style::default(),
            git_core::DiffLineKind::Added => filled(p.success.scale_alpha(alpha)),
            git_core::DiffLineKind::Removed => filled(p.danger.scale_alpha(alpha)),
        };
        if selected {
            container::Style {
                border: Border::default().width(1).color(p.accent.scale_alpha(0.8)),
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
    move |theme| {
        let p = palette(theme);
        let color = if ours { p.accent } else { p.warning };
        filled(color.scale_alpha(if picked { 0.32 } else { 0.12 }))
    }
}

/// Outline of the branch a dragged branch would be dropped on (M4-11).
pub fn drop_target(theme: &Theme) -> container::Style {
    let accent = palette(theme).accent;
    container::Style {
        border: Border::default().width(2).color(accent).rounded(4),
        background: Some(accent.scale_alpha(0.15).into()),
        ..container::Style::default()
    }
}

/// Selected segment of a segmented control (diff modes).
pub fn mode_active(theme: &Theme, _: button::Status) -> button::Style {
    let p = palette(theme);
    button_style(Some(p.bg_selected), p.text)
}

/// Toolbar pickers: readable text even while they cannot be opened yet.
/// Empty half of a Split diff row (M7-04).
pub fn diff_empty_side(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.bg_panel.scale_alpha(0.6))),
        ..container::Style::default()
    }
}

/// Bar above a change block in the Inline diff view (M7-03).
pub fn diff_block_bar(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.bg_panel)),
        border: Border::default().width(0),
        text_color: Some(p.text_muted),
        ..container::Style::default()
    }
}

pub fn picker(theme: &Theme, status: button::Status) -> button::Style {
    let p = palette(theme);
    match status {
        button::Status::Hovered | button::Status::Pressed => button_style(Some(p.bg_hover), p.text),
        button::Status::Disabled => button_style(None, p.text_muted),
        button::Status::Active => button_style(None, p.text),
    }
}

/// App-wide informational banner.
pub fn notice_banner(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.warning.scale_alpha(0.16))),
        border: Border::default().width(1).color(p.warning.scale_alpha(0.5)),
        text_color: Some(p.text),
        ..container::Style::default()
    }
}

/// Operation in progress (merge, rebase, ...) at the top of a tab.
pub fn state_banner(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.accent.scale_alpha(0.16))),
        border: Border::default().width(1).color(p.accent.scale_alpha(0.5)),
        text_color: Some(p.text),
        ..container::Style::default()
    }
}

/// Inline text input (e.g. new branch name); red border on an error.
/// Text input with a clearly visible focus ring (M6-07): 2 px accent border.
pub fn input(
    theme: &Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    let mut style = iced::widget::text_input::default(theme, status);
    if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
        style.border = style.border.color(palette(theme).accent).width(2);
    }
    style
}

/// Multi-line editor with the same focus ring as [`input`].
pub fn editor(
    theme: &Theme,
    status: iced::widget::text_editor::Status,
) -> iced::widget::text_editor::Style {
    let mut style = iced::widget::text_editor::default(theme, status);
    if matches!(status, iced::widget::text_editor::Status::Focused { .. }) {
        style.border = style.border.color(palette(theme).accent).width(2);
    }
    style
}

pub fn inline_input(
    error: bool,
) -> impl Fn(&Theme, iced::widget::text_input::Status) -> iced::widget::text_input::Style {
    move |theme, status| {
        let mut style = input(theme, status);
        if error {
            style.border = style.border.color(palette(theme).danger).width(1);
        }
        style
    }
}

/// Small dropdown arrow next to a toolbar button.
pub fn dropdown_arrow(theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(palette(theme).text_muted),
        border: Border::default().rounded(3),
        ..container::Style::default()
    }
}

/// Dimmed backdrop behind a modal dialog.
pub fn backdrop(theme: &Theme) -> container::Style {
    filled(palette(theme).backdrop)
}

pub fn dialog(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.bg_panel)),
        border: Border::default().rounded(6).width(1).color(p.border),
        text_color: Some(p.text),
        ..container::Style::default()
    }
}

pub fn dialog_details(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.bg_chrome)),
        border: Border::default().rounded(3).width(1).color(p.border),
        text_color: Some(p.text),
        ..container::Style::default()
    }
}

pub fn menu(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.bg_panel)),
        border: Border::default().rounded(4).width(1).color(p.border),
        shadow: iced::Shadow {
            color: Color::BLACK.scale_alpha(0.4),
            offset: iced::Vector::new(0.0, 3.0),
            blur_radius: 10.0,
        },
        text_color: Some(p.text),
        ..container::Style::default()
    }
}

pub fn menu_separator(theme: &Theme) -> container::Style {
    filled(palette(theme).border)
}

/// Disabled palette entry: still clickable (it shows its reason), muted.
pub fn menu_item_disabled(theme: &Theme, status: button::Status) -> button::Style {
    let p = palette(theme);
    match status {
        button::Status::Hovered | button::Status::Pressed => {
            button_style(Some(p.bg_hover), p.text_muted)
        }
        _ => button_style(None, p.text_muted),
    }
}

pub fn menu_item(theme: &Theme, status: button::Status) -> button::Style {
    let p = palette(theme);
    match status {
        button::Status::Hovered | button::Status::Pressed => {
            button_style(Some(p.bg_selected), p.text)
        }
        button::Status::Disabled => button_style(None, p.text_muted.scale_alpha(0.6)),
        button::Status::Active => button_style(None, p.text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance.
    fn luminance(c: Color) -> f32 {
        let channel = |v: f32| {
            if v <= 0.039_28 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
    }

    fn contrast(a: Color, b: Color) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn text_reaches_wcag_aa_in_all_palettes() {
        for (name, p) in [
            ("dark", &DARK),
            ("light", &LIGHT),
            ("classic", &CLASSIC_DARK),
        ] {
            for (bg_name, bg) in [
                ("graph", p.bg_graph),
                ("panel", p.bg_panel),
                ("chrome", p.bg_chrome),
                ("hover", p.bg_hover),
                ("selected", p.bg_selected),
            ] {
                let ratio = contrast(p.text, bg);
                assert!(ratio >= 4.5, "{name}: text on {bg_name} {ratio:.2}");
            }
            // Muted text is used on the plain backgrounds, not on selections.
            for (bg_name, bg) in [
                ("graph", p.bg_graph),
                ("panel", p.bg_panel),
                ("chrome", p.bg_chrome),
            ] {
                let ratio = contrast(p.text_muted, bg);
                assert!(ratio >= 4.5, "{name}: muted text on {bg_name} {ratio:.2}");
            }
        }
        // Messages in danger/warning colors are text too.
        for color in [LIGHT.danger, LIGHT.warning, LIGHT.success, LIGHT.accent] {
            assert!(contrast(color, LIGHT.bg_panel) >= 4.5);
        }
    }

    #[test]
    fn lanes_stay_visible_and_distinct() {
        for (name, p) in [
            ("dark", &DARK),
            ("light", &LIGHT),
            ("classic", &CLASSIC_DARK),
        ] {
            for (i, lane) in p.lanes.iter().enumerate() {
                // WCAG 1.4.11: graphical objects need 3:1.
                let ratio = contrast(*lane, p.bg_graph);
                assert!(ratio >= 3.0, "{name}: lane {i} {ratio:.2}");
                for other in &p.lanes[i + 1..] {
                    assert_ne!(lane, other);
                }
            }
        }
    }

    #[test]
    fn palette_follows_the_theme() {
        assert_eq!(palette(&app_theme(ThemeKind::Dark)), &DARK);
        assert_eq!(palette(&app_theme(ThemeKind::Light)), &LIGHT);
        assert_eq!(palette(&app_theme(ThemeKind::ClassicDark)), &CLASSIC_DARK);
    }

    #[test]
    fn dark_and_light_keep_their_look() {
        // The tokens reproduce the look before M11 (ADR-048).
        for p in [&DARK, &LIGHT] {
            assert_eq!(p.row_tint_alpha, 0.0);
            assert_eq!(p.ref_pill, PillStyle::Outline);
            assert!(!p.ref_connector && !p.panel_shadow);
            assert_eq!(p.header_band, p.bg_panel);
            assert_eq!(p.column_separator, Color::TRANSPARENT);
            assert_eq!(p.node_style, NodeStyle::Standard);
        }
    }
}
