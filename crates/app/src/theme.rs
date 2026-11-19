//! Colors and widget styles. All colors come from a [`ThemePalette`] chosen
//! at runtime (ADR-027); style functions take it from the iced [`Theme`]
//! they are called with, so a theme switch applies everywhere at once.

use git_core::{ChangeKind, RefKind};
use std::sync::Arc;

use iced::theme::Palette;
use iced::widget::{button, container, pane_grid, text};
use iced::{Background, Border, Color, Theme};

pub(crate) const fn hex(value: u32) -> Color {
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
    /// Framed areas inside a panel (the details' file list, finding #42):
    /// a little darker than the panel in both modes.
    pub bg_inset: Color,
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
    /// Git flow layout colors (M13-02, ADR-048 token; ADR-058).
    pub flow: FlowColors,
    /// Changed characters within an added or removed line (word diff, fix
    /// after M15; ADR-048 tokens), drawn over the line's background.
    pub diff_added_emphasis: Color,
    pub diff_removed_emphasis: Color,
}

/// Git flow layout colors (ADR-058): one per branch type, and a list of
/// one to four colors for feature branches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowColors {
    pub production: Color,
    pub hotfix: Color,
    pub release: Color,
    pub development: Color,
    pub unknown: Color,
    pub features: FeatureColors,
}

/// One to four feature branch colors: with one, every feature branch has
/// it; with several they are cycled (ADR-058).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeatureColors {
    colors: [Color; FeatureColors::MAX],
    len: usize,
}

impl FeatureColors {
    pub const MAX: usize = 4;

    /// Four colors (the built-in themes).
    pub const fn four(colors: [Color; 4]) -> Self {
        Self { colors, len: 4 }
    }

    /// `colors` when there are one to four of them.
    #[allow(dead_code)] // Used by the built-in themes and theme files (M16-04, M17).
    pub fn new(colors: &[Color]) -> Option<Self> {
        if colors.is_empty() || colors.len() > Self::MAX {
            return None;
        }
        let mut all = [colors[0]; Self::MAX];
        all[..colors.len()].copy_from_slice(colors);
        Some(Self {
            colors: all,
            len: colors.len(),
        })
    }

    #[allow(dead_code)] // Used by the built-in themes and theme files (M16-04, M17).
    pub fn as_slice(&self) -> &[Color] {
        &self.colors[..self.len]
    }

    /// Color of the feature color id `index` of `git-graph`.
    pub fn get(&self, index: usize) -> Color {
        self.colors[index % self.len.max(1)]
    }
}

impl FlowColors {
    /// Color of a Git flow color id (`git_graph::flow`): 0 production,
    /// 1 hotfix, 2 release, 3 development, 4 … 7 features, 8 unknown.
    pub fn get(&self, id: u32) -> Color {
        match id {
            0 => self.production,
            1 => self.hotfix,
            2 => self.release,
            3 => self.development,
            4..=7 => self.features.get(id as usize - 4),
            _ => self.unknown,
        }
    }
}

#[allow(dead_code)] // Used by the built-in themes and theme files (M16-04, M17).
/// The ten colors that define a theme (ADR-058); [`ThemePalette::derive`]
/// computes the others.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MainColors {
    /// Background (`bg_graph`).
    pub background: Color,
    /// Surface (`bg_panel`).
    pub surface: Color,
    /// Chrome (`bg_chrome`).
    pub chrome: Color,
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub border: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
}

/// The themes' bases (ADR-058): every theme is dark or light.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    Dark,
    Light,
}

pub const DARK: ThemePalette = ThemePalette {
    bg_graph: hex(0x1f232b),
    bg_panel: hex(0x252a33),
    bg_inset: hex(0x1f232b),
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
    flow: FlowColors {
        production: hex(0x5fb878),  // green
        hotfix: hex(0x56b6c2),      // cyan
        release: hex(0x4f9dde),     // blue
        development: hex(0xc678dd), // violet
        unknown: hex(0x8a93a3),     // gray
        // Pink, red, orange, yellow.
        features: FeatureColors::four([hex(0xe86fb0), hex(0xe06c75), hex(0xd19a66), hex(0xe5c07b)]),
    },
    diff_added_emphasis: Color::from_rgba(0.373, 0.722, 0.471, 0.36),
    diff_removed_emphasis: Color::from_rgba(0.878, 0.424, 0.459, 0.36),
};

pub const LIGHT: ThemePalette = ThemePalette {
    bg_graph: hex(0xffffff),
    bg_panel: hex(0xf4f5f7),
    bg_inset: hex(0xeceef1),
    bg_chrome: hex(0xe8eaee),
    // Visible on `bg_inset` as well (finding #47).
    bg_hover: hex(0xdde2ea),
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
    flow: FlowColors {
        production: hex(0x2e8b3a),  // green
        hotfix: hex(0x0f8b9c),      // cyan
        release: hex(0x1f6fc5),     // blue
        development: hex(0x8e44ad), // violet
        unknown: hex(0x6b7280),     // gray
        // Pink, red, orange, yellow.
        features: FeatureColors::four([hex(0xc2307f), hex(0xd1343f), hex(0xc0601c), hex(0xa86d0c)]),
    },
    diff_added_emphasis: Color::from_rgba(0.180, 0.490, 0.196, 0.26),
    diff_removed_emphasis: Color::from_rgba(0.776, 0.157, 0.157, 0.22),
};

#[allow(dead_code)] // Used by the built-in themes and theme files (M16-04, M17).
impl ThemePalette {
    /// The ten main colors of the palette.
    pub fn main_colors(&self) -> MainColors {
        MainColors {
            background: self.bg_graph,
            surface: self.bg_panel,
            chrome: self.bg_chrome,
            text: self.text,
            text_muted: self.text_muted,
            accent: self.accent,
            border: self.border,
            success: self.success,
            warning: self.warning,
            danger: self.danger,
        }
    }

    /// A palette from the ten main colors (ADR-058): inset, hover,
    /// selected, header band, backdrop and the diff emphasis colors are
    /// computed; lanes and Git flow colors are those of the base's default
    /// theme, node text is black or white by contrast. Deterministic.
    pub fn derive(main: MainColors, base: ThemeKind) -> Self {
        let defaults = match base {
            ThemeKind::Dark => &DARK,
            ThemeKind::Light => &LIGHT,
        };
        let (hover, selected, backdrop, emphasis) = match base {
            ThemeKind::Dark => (0.08, 0.25, 0.55, 0.36),
            ThemeKind::Light => (0.10, 0.20, 0.35, 0.24),
        };
        let palette = Self {
            bg_graph: main.background,
            bg_panel: main.surface,
            // Between the panel and the chrome: a little darker than the
            // panel (finding #42).
            bg_inset: mix(main.surface, main.chrome, 0.5),
            bg_chrome: main.chrome,
            bg_hover: mix(main.surface, main.text, hover),
            bg_selected: mix(main.background, main.accent, selected),
            border: main.border,
            text: main.text,
            text_muted: main.text_muted,
            accent: main.accent,
            success: main.success,
            warning: main.warning,
            danger: main.danger,
            node_text: Color::WHITE,
            lanes: defaults.lanes,
            backdrop: Color::BLACK.scale_alpha(backdrop),
            header_band: main.surface,
            flow: defaults.flow,
            diff_added_emphasis: main.success.scale_alpha(emphasis),
            diff_removed_emphasis: main.danger.scale_alpha(emphasis),
        };
        palette.with_graph(defaults.lanes, defaults.flow)
    }

    /// The palette with other lane and Git flow colors; the node text is
    /// chosen again (black or white, whichever reads better on the lanes).
    pub fn with_graph(self, lanes: [Color; 8], flow: FlowColors) -> Self {
        Self {
            lanes,
            flow,
            node_text: node_text_for(&lanes),
            ..self
        }
    }
}

#[allow(dead_code)] // Used by the built-in themes and theme files (M16-04, M17).
/// Black or white, whichever has the better worst contrast on `fills`.
pub fn node_text_for(fills: &[Color]) -> Color {
    let worst = |text: Color| {
        fills
            .iter()
            .map(|fill| contrast(text, *fill))
            .fold(f32::INFINITY, f32::min)
    };
    if worst(Color::BLACK) > worst(Color::WHITE) {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

/// Palette of the theme a style function is called with: the registry
/// entry named like the iced theme (ADR-058); other iced themes (tests use
/// `Theme::Dark` and `Theme::Light`) get Dark or Light by their brightness.
pub fn palette(theme: &Theme) -> Arc<ThemePalette> {
    crate::themes::palette_of(iced::theme::Base::name(theme)).unwrap_or_else(|| {
        let base = if theme.extended_palette().is_dark {
            ThemeKind::Dark
        } else {
            ThemeKind::Light
        };
        Arc::clone(&crate::themes::default_of(base).palette)
    })
}

/// iced theme of the base's default theme (`dark` or `light`).
#[cfg(test)]
pub fn app_theme(kind: ThemeKind) -> Theme {
    iced_theme(&crate::themes::default_of(kind))
}

/// iced theme for a registry theme: named by the theme id (so
/// [`palette`] finds it) and dark or light by the theme's base, whatever
/// its background looks like.
pub fn iced_theme(def: &crate::themes::ThemeDef) -> Theme {
    let p = &def.palette;
    let dark = def.base == ThemeKind::Dark;
    Theme::custom_with_fn(
        def.id.clone(),
        Palette {
            background: p.bg_graph,
            text: p.text,
            primary: p.accent,
            success: p.success,
            warning: p.warning,
            danger: p.danger,
        },
        move |palette| iced::theme::palette::Extended {
            is_dark: dark,
            ..iced::theme::palette::Extended::generate(palette)
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
        color: Some(pick(&palette(theme))),
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
pub fn luminance(c: Color) -> f32 {
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
pub fn contrast(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `a` moved towards `b` by `t` (0 … 1).
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgb(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}

/// Accent alpha of a start screen button (M15-07): like the initials tile
/// of the commit details (`avatar`, 25 %), stronger on hover and press.
fn start_button_alpha(status: button::Status) -> f32 {
    match status {
        button::Status::Hovered => 0.35,
        button::Status::Pressed => 0.45,
        button::Status::Active | button::Status::Disabled => 0.25,
    }
}

/// Fill of a start screen button (M15-07, calmer than the solid accent of
/// M11-12): the accent at 25 / 35 / 45 % alpha.
pub fn start_button_fill(p: &ThemePalette, status: button::Status) -> Color {
    p.accent.scale_alpha(start_button_alpha(status))
}

/// The fill as it looks on the start screen's background (for contrast).
#[cfg(test)]
fn start_button_on_background(p: &ThemePalette, status: button::Status) -> Color {
    mix(p.bg_graph, p.accent, start_button_alpha(status))
}

/// Large start screen buttons "Open a repo", "Clone a repo", "Start a
/// local repo" (M11-12, M15-07): title, hint and icon in the text color.
pub fn start_button(theme: &Theme, status: button::Status) -> button::Style {
    let p = palette(theme);
    button::Style {
        background: Some(Background::Color(start_button_fill(&p, status))),
        text_color: p.text,
        border: Border::default().rounded(6),
        ..button::Style::default()
    }
}

/// Icon color on a start screen button: the text color (M15-07).
pub fn start_button_icon(theme: &Theme) -> Color {
    palette(theme).text
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

/// Text button of the status bar (the language item, ADR-057): no
/// background in any state like the pin button; muted, the text color on
/// hover.
pub fn status_item(theme: &Theme, status: button::Status) -> button::Style {
    let p = palette(theme);
    let color = match status {
        button::Status::Hovered | button::Status::Pressed => p.text,
        _ => p.text_muted,
    };
    button_style(None, color)
}

pub fn section_header(theme: &Theme, status: button::Status) -> button::Style {
    // The title is always in the full text color (fix/sidebar-section-
    // headers); hovering shows a background like a list item.
    let p = palette(theme);
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => Some(p.bg_hover),
        _ => None,
    };
    button::Style {
        border: Border::default(),
        ..button_style(background, p.text)
    }
}

/// Faded label of a deleted branch in the Git flow layout (M13-04): a
/// thin muted outline without fill (iced draws no dashed borders).
pub fn flow_pill(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        border: Border::default()
            .rounded(3)
            .width(1)
            .color(p.text_muted.scale_alpha(0.6)),
        text_color: Some(p.text_muted),
        ..container::Style::default()
    }
}

/// Time marker pill at the right of the commit message (M14-06): a muted
/// pill (hover color, small text); the text keeps the normal color because
/// muted text on it misses WCAG AA.
pub fn time_marker(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.bg_hover)),
        border: Border::default().rounded(8),
        text_color: Some(p.text),
        ..container::Style::default()
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

/// Palette color for a lane color id from `git-graph`; ids from
/// [`git_graph::flow::FLOW_COLOR_BASE`] on are Git flow colors (M13-02).
pub fn lane_color(p: &ThemePalette, color: u32) -> Color {
    match color.checked_sub(git_graph::flow::FLOW_COLOR_BASE) {
        Some(flow) => p.flow.get(flow),
        None => p.lanes[color as usize % p.lanes.len()],
    }
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
            (true, _) => Some(selected_row(&p, lane)),
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
        let color = toast_color(&p, kind);
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

/// Background of the changed characters of a `kind` line (word diff, fix
/// after M15) in the code views' style; none for context lines.
pub fn diff_emphasis(
    style: &crate::editor_style::EditorStyle,
    kind: git_core::DiffLineKind,
) -> Option<Color> {
    match kind {
        git_core::DiffLineKind::Added => Some(style.added_emphasis),
        git_core::DiffLineKind::Removed => Some(style.removed_emphasis),
        git_core::DiffLineKind::Context => None,
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
/// Frame of an icon segmented toggle (diff view modes, M15-08).
pub fn segmented(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        border: Border {
            color: p.border,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..container::Style::default()
    }
}

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

/// Framed box of the commit message in the details (M14-01).
pub fn message_box(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.bg_graph)),
        border: Border::default().rounded(4).width(1).color(p.border),
        text_color: Some(p.text),
        ..container::Style::default()
    }
}

/// Frame of the details' file list (finding #42): border like the
/// message box, a background a little darker than the panel.
pub fn file_frame(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.bg_inset)),
        border: Border::default().rounded(4).width(1).color(p.border),
        text_color: Some(p.text),
        ..container::Style::default()
    }
}

/// Read-only text inside [`message_box`]: no frame of its own.
pub fn message_text(
    theme: &Theme,
    status: iced::widget::text_editor::Status,
) -> iced::widget::text_editor::Style {
    let p = palette(theme);
    iced::widget::text_editor::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        placeholder: p.text_muted,
        value: p.text,
        ..iced::widget::text_editor::default(theme, status)
    }
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

    /// Fix/sidebar-section-headers: the title keeps the full text color;
    /// only hovering (and pressing) adds the list item background.
    #[test]
    fn section_headers_keep_the_text_color_and_show_hover() {
        for (theme, p) in [(Theme::Dark, &DARK), (Theme::Light, &LIGHT)] {
            for status in [
                button::Status::Active,
                button::Status::Hovered,
                button::Status::Pressed,
            ] {
                let style = section_header(&theme, status);
                assert_eq!(style.text_color, p.text);
                let hover = status != button::Status::Active;
                assert_eq!(
                    style.background,
                    hover.then_some(Background::Color(p.bg_hover)),
                    "{status:?}"
                );
            }
        }
    }

    #[test]
    fn start_buttons_reach_wcag_aa() {
        for (name, p) in [("dark", &DARK), ("light", &LIGHT)] {
            for status in [
                button::Status::Active,
                button::Status::Hovered,
                button::Status::Pressed,
            ] {
                // Title, hint and icon use the text color (M15-07).
                let fill = start_button_on_background(p, status);
                let ratio = contrast(p.text, fill);
                assert!(ratio >= 4.5, "{name} {status:?}: {ratio:.2}");
            }
        }
    }

    /// Finding #47: rows in a file frame (`bg_inset`): text stays readable
    /// on the frame, a hovered and a selected row, and hover and selection
    /// stand out from the frame at least as much as hover does on the
    /// panel.
    #[test]
    fn file_frame_rows_stay_readable() {
        for (name, p) in [("dark", &DARK), ("light", &LIGHT)] {
            for (fg_name, fg, bg_name, bg) in [
                ("text", p.text, "inset", p.bg_inset),
                ("muted", p.text_muted, "inset", p.bg_inset),
                ("text", p.text, "hover", p.bg_hover),
                ("text", p.text, "selected", p.bg_selected),
            ] {
                let ratio = contrast(fg, bg);
                assert!(ratio >= 4.5, "{name}: {fg_name} on {bg_name} {ratio:.2}");
            }
            for (bg_name, bg) in [("hover", p.bg_hover), ("selected", p.bg_selected)] {
                let ratio = contrast(bg, p.bg_inset);
                assert!(ratio >= 1.1, "{name}: {bg_name} on inset {ratio:.3}");
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
        // Time marker pills (M14-06).
        for kind in [ThemeKind::Dark, ThemeKind::Light] {
            let style = time_marker(&app_theme(kind));
            let (Some(fg), Some(Background::Color(bg))) = (style.text_color, style.background)
            else {
                panic!("time marker without colors");
            };
            assert!(
                contrast(fg, bg) >= 4.5,
                "time marker {:.2}",
                contrast(fg, bg)
            );
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
    fn flow_colors_stay_visible() {
        for (name, p) in [("dark", &DARK), ("light", &LIGHT)] {
            let flow = &p.flow;
            let all = [
                flow.production,
                flow.hotfix,
                flow.release,
                flow.development,
                flow.unknown,
            ];
            for (i, color) in all.iter().chain(flow.features.as_slice()).enumerate() {
                let ratio = contrast(*color, p.bg_graph);
                assert!(ratio >= 3.0, "{name}: flow color {i} {ratio:.2}");
            }
            let base = git_graph::flow::FLOW_COLOR_BASE;
            assert_eq!(lane_color(p, base + 3), p.flow.development);
            assert_eq!(lane_color(p, 3), p.lanes[3]);
        }
    }

    #[test]
    fn palette_follows_the_theme() {
        assert_eq!(*palette(&app_theme(ThemeKind::Dark)), DARK);
        assert_eq!(*palette(&app_theme(ThemeKind::Light)), LIGHT);
    }

    #[test]
    fn dark_and_light_keep_their_look() {
        // The header band is the panel color (ADR-048, ADR-054).
        for p in [&DARK, &LIGHT] {
            assert_eq!(p.header_band, p.bg_panel);
        }
    }

    /// ADR-058: feature color ids base+4 … base+7 map to `features[i % len]`.
    #[test]
    fn feature_colors_cycle_through_the_list() {
        let base = git_graph::flow::FLOW_COLOR_BASE;
        let c = [hex(0x111111), hex(0x222222), hex(0x333333), hex(0x444444)];
        for len in 1..=4 {
            let features = FeatureColors::new(&c[..len]).expect("1 … 4");
            assert_eq!(features.as_slice(), &c[..len]);
            let p = ThemePalette {
                flow: FlowColors {
                    features,
                    ..DARK.flow
                },
                ..DARK
            };
            let got: Vec<Color> = (4..8).map(|id| lane_color(&p, base + id)).collect();
            let want: Vec<Color> = (0..4).map(|i| c[i % len]).collect();
            assert_eq!(got, want, "{len} colors");
        }
        // One entry: every feature branch has the same color.
        let one = FeatureColors::new(&c[..1]).expect("one");
        assert!((0..4).all(|i| one.get(i) == c[0]));
        assert!(FeatureColors::new(&[]).is_none());
        assert!(FeatureColors::new(&[c[0]; 5]).is_none());
        // The other types keep their colors.
        assert_eq!(lane_color(&DARK, base), DARK.flow.production);
        assert_eq!(lane_color(&DARK, base + 1), DARK.flow.hotfix);
        assert_eq!(lane_color(&DARK, base + 2), DARK.flow.release);
        assert_eq!(lane_color(&DARK, base + 8), DARK.flow.unknown);
    }

    /// ADR-058: derived backgrounds keep text readable (4.5:1) for sample
    /// main colors of both bases.
    #[test]
    fn derived_backgrounds_keep_text_readable() {
        let samples = [
            (DARK.main_colors(), ThemeKind::Dark),
            (LIGHT.main_colors(), ThemeKind::Light),
            (
                MainColors {
                    background: hex(0x262624),
                    surface: hex(0x30302e),
                    chrome: hex(0x1f1e1d),
                    text: hex(0xf0eee6),
                    text_muted: hex(0xb0aea5),
                    accent: hex(0xd97757),
                    border: hex(0x3e3e38),
                    success: hex(0x7fb069),
                    warning: hex(0xe0b050),
                    danger: hex(0xe06c60),
                },
                ThemeKind::Dark,
            ),
            (
                MainColors {
                    background: hex(0xfdf6e3),
                    surface: hex(0xeee8d5),
                    chrome: hex(0xe4ddc8),
                    text: hex(0x073642),
                    text_muted: hex(0x586e75),
                    accent: hex(0x268bd2),
                    border: hex(0xd3cbb7),
                    success: hex(0x5f7d00),
                    warning: hex(0x8a6100),
                    danger: hex(0xc0302c),
                },
                ThemeKind::Light,
            ),
        ];
        for (i, (main, base)) in samples.into_iter().enumerate() {
            let p = ThemePalette::derive(main, base);
            assert_eq!(p.main_colors(), main);
            for (name, bg) in [
                ("inset", p.bg_inset),
                ("hover", p.bg_hover),
                ("selected", p.bg_selected),
                ("header band", p.header_band),
            ] {
                let ratio = contrast(p.text, bg);
                assert!(ratio >= 4.5, "sample {i}: text on {name} {ratio:.2}");
            }
            assert_ne!(p.bg_hover, p.bg_panel, "sample {i}");
            assert_ne!(p.bg_selected, p.bg_graph, "sample {i}");
            assert_eq!(ThemePalette::derive(main, base), p, "deterministic");
        }
    }

    #[test]
    fn node_text_reads_best_on_the_lanes() {
        assert_eq!(node_text_for(&LIGHT.lanes), Color::WHITE);
        assert_eq!(node_text_for(&[hex(0xf5f5a0), hex(0xa0f5f5)]), Color::BLACK);
    }
}
