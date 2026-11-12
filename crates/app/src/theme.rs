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
    /// Background of the graph's column header (ADR-048 token).
    pub header_band: Color,
}

/// The themes (ADR-054: Dark and Light).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    Dark,
    Light,
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
    header_band: hex(0x252a33),
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
    header_band: hex(0xf4f5f7),
};

/// Palette of the theme a style function is called with.
pub fn palette(theme: &Theme) -> &'static ThemePalette {
    if theme.extended_palette().is_dark {
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
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
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

/// WCAG relative luminance.
pub(crate) fn luminance(c: Color) -> f32 {
    let channel = |v: f32| {
        if v <= 0.039_28 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}

/// WCAG contrast ratio of two colors.
pub(crate) fn contrast(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `a` moved towards `b` by `t` (0 … 1).
fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgb(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}

/// Dark text on light fills.
const ON_LIGHT: Color = Color::from_rgb(0.067, 0.078, 0.094);

/// Text on a filled background: white or near-black, whichever has more
/// contrast.
pub fn on_fill(fill: Color) -> Color {
    if contrast(Color::WHITE, fill) >= contrast(ON_LIGHT, fill) {
        Color::WHITE
    } else {
        ON_LIGHT
    }
}

/// Fill of a start screen button (M11-12): the accent color (as the recent
/// repository links); hover and press move it away from the text color,
/// so the text keeps its contrast and color in every state.
pub fn start_button_fill(p: &ThemePalette, status: button::Status) -> Color {
    let away = if on_fill(p.accent) == Color::WHITE {
        Color::BLACK
    } else {
        Color::WHITE
    };
    match status {
        button::Status::Hovered => mix(p.accent, away, 0.12),
        button::Status::Pressed => mix(p.accent, away, 0.24),
        button::Status::Active | button::Status::Disabled => p.accent,
    }
}

/// Large start screen buttons "Open a repo", "Clone a repo", "Start a
/// local repo" (M11-12).
pub fn start_button(theme: &Theme, status: button::Status) -> button::Style {
    let p = palette(theme);
    button::Style {
        background: Some(Background::Color(start_button_fill(p, status))),
        text_color: on_fill(p.accent),
        border: Border::default().rounded(6),
        ..button::Style::default()
    }
}

/// Icon color on a start screen button.
pub fn start_button_icon(theme: &Theme) -> Color {
    on_fill(palette(theme).accent)
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

/// A button without background or border in every state (the status bar
/// panel toggles; their icon shows hover and state).
pub fn bare(theme: &Theme, _status: button::Status) -> button::Style {
    button_style(None, palette(theme).text)
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

/// Ref pill (branch or tag label): outline in the kind's color.
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

/// Graph table row; the selected row is tinted with its lane color.
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
/// A toast (M12-02): the panel color with a stripe in the kind's color.
pub fn toast(kind: crate::toast::ToastKind) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let p = palette(theme);
        let color = toast_color(p, kind);
        container::Style {
            background: Some(Background::Color(p.bg_panel)),
            border: Border::default().rounded(6).width(1).color(color),
            text_color: Some(p.text),
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 8.0,
            },
            ..container::Style::default()
        }
    }
}

/// Color of a toast kind.
pub fn toast_color(p: &ThemePalette, kind: crate::toast::ToastKind) -> Color {
    use crate::toast::ToastKind;
    match kind {
        ToastKind::Error => p.danger,
        ToastKind::Warning => p.warning,
        ToastKind::Info => p.accent,
        ToastKind::Success => p.success,
    }
}

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

/// Operation in progress (M12-05): the graph row and the details header,
/// in the warning color like GitKraken's orange row.
pub fn operation_row(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.warning.scale_alpha(0.22))),
        border: Border::default().width(1).color(p.warning.scale_alpha(0.6)),
        text_color: Some(p.text),
        ..container::Style::default()
    }
}

/// A Preferences field opened from the palette (M12-12): accent tint.
pub fn setting_highlight(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.accent.scale_alpha(0.16))),
        border: Border::default()
            .width(1)
            .rounded(4)
            .color(p.accent.scale_alpha(0.6)),
        ..container::Style::default()
    }
}

/// Git flow finish waiting at the top of a tab (M7-10).
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

    #[test]
    fn start_buttons_reach_wcag_aa() {
        for (name, p) in [("dark", &DARK), ("light", &LIGHT)] {
            let icon = on_fill(p.accent);
            for status in [
                button::Status::Active,
                button::Status::Hovered,
                button::Status::Pressed,
            ] {
                let fill = start_button_fill(p, status);
                let text = on_fill(p.accent);
                // Label and shortcut line use the same color.
                let ratio = contrast(text, fill);
                assert!(ratio >= 4.5, "{name} {status:?}: {ratio:.2}");
                // The icon has the text color.
                assert_eq!(icon, text, "{name} {status:?}");
                assert_eq!(on_fill(fill), text, "{name} {status:?}: best text color");
            }
        }
    }

    #[test]
    fn text_reaches_wcag_aa_in_all_palettes() {
        for (name, p) in [("dark", &DARK), ("light", &LIGHT)] {
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
        for (name, p) in [("dark", &DARK), ("light", &LIGHT)] {
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
    }

    #[test]
    fn dark_and_light_keep_their_look() {
        // The header band is the panel color (ADR-048, ADR-054).
        for p in [&DARK, &LIGHT] {
            assert_eq!(p.header_band, p.bg_panel);
        }
    }
}
