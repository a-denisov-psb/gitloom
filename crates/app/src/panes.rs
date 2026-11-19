//! Pane layout: sidebar | center | details panel, with hideable side
//! panels. The details panel sits right of the center or below it
//! (finding #44, ADR-056).

use iced::widget::pane_grid::{self, Axis, Configuration, Node};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    Sidebar,
    Center,
    Panel,
}

/// Where the details panel sits (`settings.json`, global; finding #44).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DetailsPanelPosition {
    #[default]
    Right,
    Bottom,
}

impl DetailsPanelPosition {
    pub fn other(self) -> Self {
        match self {
            Self::Right => Self::Bottom,
            Self::Bottom => Self::Right,
        }
    }

    /// Label of the action that moves the panel away from `self`.
    pub fn move_label(self) -> &'static str {
        match self {
            Self::Right => "Move details panel to the bottom",
            Self::Bottom => "Move details panel to the right",
        }
    }
}

fn default_bottom() -> f32 {
    0.55
}

fn default_staging() -> f32 {
    0.5
}

/// Split ratios, persisted in the session.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PaneRatios {
    /// Sidebar width as fraction of the window.
    pub sidebar: f32,
    /// Center width as fraction of center + details panel (panel right).
    pub center: f32,
    /// Graph height as fraction of center + details panel (panel at the
    /// bottom, finding #44).
    #[serde(default = "default_bottom")]
    pub bottom: f32,
    /// Height of the file lists as fraction of lists + commit message in
    /// the staging area at the bottom (finding #44).
    #[serde(default = "default_staging")]
    pub staging: f32,
}

impl Default for PaneRatios {
    fn default() -> Self {
        Self {
            sidebar: 0.18,
            center: 0.72,
            bottom: default_bottom(),
            staging: default_staging(),
        }
    }
}

impl PaneRatios {
    /// Clamps ratios read from disk to sane values.
    pub fn sanitized(self) -> Self {
        let clamp = |r: f32, default: f32| {
            if r.is_finite() {
                r.clamp(0.05, 0.95)
            } else {
                default
            }
        };
        let defaults = Self::default();
        Self {
            sidebar: clamp(self.sidebar, defaults.sidebar),
            center: clamp(self.center, defaults.center),
            bottom: clamp(self.bottom, defaults.bottom),
            staging: clamp(self.staging, defaults.staging),
        }
    }

    /// Ratio of the split between the center and the details panel at
    /// `position`.
    pub fn panel_split(&self, position: DetailsPanelPosition) -> f32 {
        match position {
            DetailsPanelPosition::Right => self.center,
            DetailsPanelPosition::Bottom => self.bottom,
        }
    }

    fn set_panel_split(&mut self, position: DetailsPanelPosition, ratio: f32) {
        match position {
            DetailsPanelPosition::Right => self.center = ratio,
            DetailsPanelPosition::Bottom => self.bottom = ratio,
        }
    }
}

/// `ratio` of a split of `total` pixels clamped so that the first part
/// keeps `min_a` and the second `min_b` pixels; when both do not fit, the
/// parts share the room in proportion to their minimums.
pub fn clamp_split(ratio: f32, total: f32, min_a: f32, min_b: f32) -> f32 {
    if !ratio.is_finite() || total <= 0.0 {
        return ratio;
    }
    if min_a + min_b >= total {
        return min_a / (min_a + min_b).max(f32::EPSILON);
    }
    ratio.clamp(min_a / total, 1.0 - min_b / total)
}

/// Edges of a pane that border a divider (finding #48).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DividerEdges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

pub struct Panes {
    pub state: pane_grid::State<PaneKind>,
    pub ratios: PaneRatios,
    pub sidebar_visible: bool,
    pub panel_visible: bool,
    pub position: DetailsPanelPosition,
    /// Room of the pane grid (logical pixels; zero until the window
    /// reported its size). Ratios are clamped against the pixel minimums
    /// of `crate::layout` for it (finding #45).
    pub area: iced::Size,
}

impl Panes {
    pub fn new(ratios: PaneRatios) -> Self {
        let mut panes = Self {
            state: pane_grid::State::new(PaneKind::Center).0,
            ratios: ratios.sanitized(),
            sidebar_visible: true,
            panel_visible: true,
            position: DetailsPanelPosition::Right,
            area: iced::Size::ZERO,
        };
        panes.rebuild();
        panes
    }

    /// Sets the room of the pane grid; the layout is rebuilt when the
    /// clamped ratios change (after a smaller window, for example).
    pub fn set_area(&mut self, area: iced::Size) {
        if self.area == area {
            return;
        }
        let before = self.clamped();
        self.area = area;
        if self.clamped() != before {
            self.rebuild();
        }
    }

    /// Sidebar ratio and the ratio between center and details panel as
    /// laid out: the stored ratios clamped so that every visible pane
    /// keeps its minimum size in `area` (finding #45).
    pub fn clamped(&self) -> (f32, f32) {
        use crate::layout::{
            BOTTOM_GRAPH_MIN_HEIGHT, BOTTOM_PANEL_MIN_HEIGHT, CENTER_MIN_WIDTH, PANEL_MIN_WIDTH,
            PANE_SPACING, SIDEBAR_MIN_WIDTH,
        };
        let split = self.ratios.panel_split(self.position);
        if self.area.width <= 0.0 || self.area.height <= 0.0 {
            return (self.ratios.sidebar, split);
        }
        let right_panel = self.panel_visible && self.position == DetailsPanelPosition::Right;
        let width = self.area.width - PANE_SPACING;
        let sidebar = if self.sidebar_visible {
            let rest = CENTER_MIN_WIDTH
                + if right_panel {
                    PANE_SPACING + PANEL_MIN_WIDTH
                } else {
                    0.0
                };
            clamp_split(self.ratios.sidebar, width, SIDEBAR_MIN_WIDTH, rest)
        } else {
            self.ratios.sidebar
        };
        let split = match self.position {
            DetailsPanelPosition::Right => {
                let room = if self.sidebar_visible {
                    width * (1.0 - sidebar) - PANE_SPACING
                } else {
                    width
                };
                clamp_split(split, room, CENTER_MIN_WIDTH, PANEL_MIN_WIDTH)
            }
            DetailsPanelPosition::Bottom => clamp_split(
                split,
                self.area.height - PANE_SPACING,
                BOTTOM_GRAPH_MIN_HEIGHT,
                BOTTOM_PANEL_MIN_HEIGHT,
            ),
        };
        (sidebar, split)
    }

    pub fn set_ratios(&mut self, ratios: PaneRatios) {
        self.ratios = ratios.sanitized();
        self.rebuild();
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
        self.rebuild();
    }

    pub fn toggle_panel(&mut self) {
        self.panel_visible = !self.panel_visible;
        self.rebuild();
    }

    /// Moves the details panel; a hidden panel only remembers the place.
    pub fn set_position(&mut self, position: DetailsPanelPosition) {
        if self.position != position {
            self.position = position;
            self.rebuild();
        }
    }

    /// Applies a drag on a split and remembers the new ratio; a pane
    /// never gets smaller than its minimum (finding #45).
    pub fn resize(&mut self, split: pane_grid::Split, ratio: f32) {
        let outer = matches!(self.state.layout(), Node::Split { id, .. } if *id == split);
        let sidebar_split = outer && self.sidebar_visible;
        let mut probe = Panes {
            state: pane_grid::State::new(PaneKind::Center).0,
            ratios: self.ratios,
            sidebar_visible: self.sidebar_visible,
            panel_visible: self.panel_visible,
            position: self.position,
            area: self.area,
        };
        if sidebar_split {
            probe.ratios.sidebar = ratio;
        } else {
            probe.ratios.set_panel_split(self.position, ratio);
        }
        let (sidebar, panel) = probe.clamped();
        let ratio = if sidebar_split { sidebar } else { panel };
        self.state.resize(split, ratio);
        self.read_ratios();
    }

    /// Edges of the pane `kind` that border a divider in the current
    /// layout (finding #48).
    pub fn divider_edges(&self, kind: PaneKind) -> DividerEdges {
        let right = self.panel_visible && self.position == DetailsPanelPosition::Right;
        let bottom = self.panel_visible && self.position == DetailsPanelPosition::Bottom;
        match kind {
            PaneKind::Sidebar => DividerEdges {
                right: true,
                ..DividerEdges::default()
            },
            PaneKind::Center => DividerEdges {
                left: self.sidebar_visible,
                right,
                bottom,
                top: false,
            },
            PaneKind::Panel => DividerEdges {
                left: right || self.sidebar_visible,
                top: bottom,
                ..DividerEdges::default()
            },
        }
    }

    /// The pane configuration for the visible panes and the position.
    pub fn configuration(&self) -> Configuration<PaneKind> {
        let pane = |kind| Box::new(Configuration::Pane(kind));
        let panel_axis = match self.position {
            DetailsPanelPosition::Right => Axis::Vertical,
            DetailsPanelPosition::Bottom => Axis::Horizontal,
        };
        let (sidebar, split) = self.clamped();
        let center_and_panel = || Configuration::Split {
            axis: panel_axis,
            ratio: split,
            a: pane(PaneKind::Center),
            b: pane(PaneKind::Panel),
        };
        match (self.sidebar_visible, self.panel_visible) {
            (true, true) => Configuration::Split {
                axis: Axis::Vertical,
                ratio: sidebar,
                a: pane(PaneKind::Sidebar),
                b: Box::new(center_and_panel()),
            },
            (true, false) => Configuration::Split {
                axis: Axis::Vertical,
                ratio: sidebar,
                a: pane(PaneKind::Sidebar),
                b: pane(PaneKind::Center),
            },
            (false, true) => center_and_panel(),
            (false, false) => Configuration::Pane(PaneKind::Center),
        }
    }

    fn rebuild(&mut self) {
        self.state = pane_grid::State::with_configuration(self.configuration());
    }

    /// Reads the ratios of the visible splits back into `ratios`.
    fn read_ratios(&mut self) {
        let Node::Split { ratio, b, .. } = self.state.layout() else {
            return;
        };
        let (ratio, b) = (*ratio, b.clone());
        match (self.sidebar_visible, self.panel_visible) {
            (true, true) => {
                self.ratios.sidebar = ratio;
                if let Node::Split { ratio: inner, .. } = b.as_ref() {
                    self.ratios.set_panel_split(self.position, *inner);
                }
            }
            (true, false) => self.ratios.sidebar = ratio,
            (false, true) => self.ratios.set_panel_split(self.position, ratio),
            (false, false) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(panes: &Panes) -> Vec<PaneKind> {
        let mut kinds: Vec<_> = panes.state.iter().map(|(_, kind)| *kind).collect();
        kinds.sort_by_key(|k| *k as u8);
        kinds
    }

    #[test]
    fn toggling_panels_keeps_ratios() {
        let ratios = PaneRatios {
            sidebar: 0.25,
            center: 0.6,
            ..PaneRatios::default()
        };
        let mut panes = Panes::new(ratios);
        assert_eq!(
            kinds(&panes),
            [PaneKind::Sidebar, PaneKind::Center, PaneKind::Panel]
        );

        panes.toggle_sidebar();
        assert_eq!(kinds(&panes), [PaneKind::Center, PaneKind::Panel]);
        panes.toggle_panel();
        assert_eq!(kinds(&panes), [PaneKind::Center]);
        panes.toggle_sidebar();
        panes.toggle_panel();
        assert_eq!(kinds(&panes).len(), 3);
        assert_eq!(panes.ratios, ratios);
    }

    #[test]
    fn resizing_updates_ratios() {
        let mut panes = Panes::new(PaneRatios::default());
        let Node::Split { id, b, .. } = panes.state.layout().clone() else {
            panic!("expected split");
        };
        panes.resize(id, 0.3);
        assert!((panes.ratios.sidebar - 0.3).abs() < 1e-6);
        let Node::Split { id: inner, .. } = *b else {
            panic!("expected inner split");
        };
        panes.resize(inner, 0.5);
        assert!((panes.ratios.center - 0.5).abs() < 1e-6);
    }

    #[test]
    fn invalid_ratios_are_sanitized() {
        let panes = Panes::new(PaneRatios {
            sidebar: f32::NAN,
            center: 7.0,
            ..PaneRatios::default()
        });
        assert_eq!(panes.ratios.sidebar, PaneRatios::default().sidebar);
        assert_eq!(panes.ratios.center, 0.95);
    }

    /// The split between the center and the panel of a configuration, as
    /// (axis, ratio), and the kinds in order.
    fn shape(config: &Configuration<PaneKind>) -> String {
        match config {
            Configuration::Pane(kind) => format!("{kind:?}"),
            Configuration::Split { axis, ratio, a, b } => {
                let axis = if *axis == Axis::Vertical { "|" } else { "/" };
                format!("({} {axis}{ratio:.2} {})", shape(a), shape(b))
            }
        }
    }

    /// Finding #48: the edges of each pane that border a divider.
    #[test]
    fn divider_edges_follow_the_layout() {
        let edges = |l, r, t, b| DividerEdges {
            left: l,
            right: r,
            top: t,
            bottom: b,
        };
        let mut panes = Panes::new(PaneRatios::default());
        let all = |p: &Panes| {
            [PaneKind::Sidebar, PaneKind::Center, PaneKind::Panel].map(|k| p.divider_edges(k))
        };
        assert_eq!(
            all(&panes),
            [
                edges(false, true, false, false),
                edges(true, true, false, false),
                edges(true, false, false, false),
            ]
        );
        panes.set_position(DetailsPanelPosition::Bottom);
        assert_eq!(
            all(&panes),
            [
                edges(false, true, false, false),
                edges(true, false, false, true),
                edges(true, false, true, false),
            ]
        );
        panes.toggle_sidebar();
        assert_eq!(
            all(&panes)[1..],
            [
                edges(false, false, false, true),
                edges(false, false, true, false)
            ]
        );
        panes.toggle_panel();
        assert_eq!(all(&panes)[1], DividerEdges::default());
    }

    #[test]
    fn configuration_per_position_and_visibility() {
        let mut panes = Panes::new(PaneRatios::default());
        let mut seen = Vec::new();
        for position in [DetailsPanelPosition::Right, DetailsPanelPosition::Bottom] {
            panes.set_position(position);
            for (sidebar, panel) in [(true, true), (true, false), (false, true), (false, false)] {
                panes.sidebar_visible = sidebar;
                panes.panel_visible = panel;
                seen.push(shape(&panes.configuration()));
            }
        }
        assert_eq!(
            seen,
            [
                "(Sidebar |0.18 (Center |0.72 Panel))",
                "(Sidebar |0.18 Center)",
                "(Center |0.72 Panel)",
                "Center",
                "(Sidebar |0.18 (Center /0.55 Panel))",
                "(Sidebar |0.18 Center)",
                "(Center /0.55 Panel)",
                "Center",
            ]
        );
    }

    #[test]
    fn each_position_keeps_its_own_ratio() {
        let mut panes = Panes::new(PaneRatios::default());
        panes.set_position(DetailsPanelPosition::Bottom);
        let Node::Split { b, .. } = panes.state.layout().clone() else {
            panic!("expected split");
        };
        let Node::Split { id: inner, .. } = *b else {
            panic!("expected inner split");
        };
        panes.resize(inner, 0.4);
        assert!((panes.ratios.bottom - 0.4).abs() < 1e-6);
        assert_eq!(panes.ratios.center, PaneRatios::default().center);
        panes.set_position(DetailsPanelPosition::Right);
        let Node::Split { b, .. } = panes.state.layout().clone() else {
            panic!("expected split");
        };
        let Node::Split { id: inner, .. } = *b else {
            panic!("expected inner split");
        };
        panes.resize(inner, 0.6);
        assert!((panes.ratios.center - 0.6).abs() < 1e-6);
        assert!((panes.ratios.bottom - 0.4).abs() < 1e-6);
        // Hidden panel: the position is remembered, the panel comes back
        // there.
        panes.toggle_panel();
        panes.set_position(DetailsPanelPosition::Bottom);
        panes.toggle_panel();
        assert_eq!(
            shape(&panes.configuration()),
            "(Sidebar |0.18 (Center /0.40 Panel))"
        );
    }

    #[test]
    fn older_ratios_without_the_new_fields_load() {
        let ratios: PaneRatios = serde_json::from_str(r#"{"sidebar":0.2,"center":0.7}"#).unwrap();
        assert_eq!(ratios.bottom, 0.55);
        assert_eq!(ratios.staging, 0.5);
        let sane = PaneRatios {
            bottom: f32::INFINITY,
            staging: 3.0,
            ..ratios
        }
        .sanitized();
        assert_eq!(sane.bottom, 0.55);
        assert_eq!(sane.staging, 0.95);
    }

    #[test]
    fn splits_keep_their_minimums() {
        assert_eq!(clamp_split(0.5, 400.0, 100.0, 100.0), 0.5);
        assert_eq!(clamp_split(0.1, 400.0, 100.0, 100.0), 0.25);
        assert_eq!(clamp_split(0.9, 400.0, 100.0, 100.0), 0.75);
        // Too small for both: in proportion to the minimums.
        assert_eq!(clamp_split(0.9, 150.0, 100.0, 50.0), 100.0 / 150.0);
        assert_eq!(clamp_split(0.9, 100.0, 100.0, 100.0), 0.5);
    }

    #[test]
    fn ratios_never_go_below_the_pixel_minimums() {
        use crate::layout::*;
        let mut panes = Panes::new(PaneRatios {
            sidebar: 0.05,
            center: 0.95,
            bottom: 0.95,
            ..PaneRatios::default()
        });
        let min = window_min();
        panes.set_area(pane_area(min));
        let widths = |panes: &Panes| {
            let (sidebar, center) = panes.clamped();
            let width = panes.area.width - PANE_SPACING;
            let side = width * sidebar;
            let rest = width - side - PANE_SPACING;
            (side, rest * center, rest * (1.0 - center))
        };
        let (side, center, panel) = widths(&panes);
        assert!(side >= SIDEBAR_MIN_WIDTH - 0.5, "{side}");
        assert!(center >= CENTER_MIN_WIDTH - 0.5, "{center}");
        assert!(panel >= PANEL_MIN_WIDTH - 0.5, "{panel}");
        // The stored preference stays; the layout uses the clamped one.
        assert_eq!(panes.ratios.sidebar, 0.05);
        assert!(shape(&panes.configuration())
            .starts_with(&format!("(Sidebar |{:.2}", panes.clamped().0)));
        // A drag below a minimum stops there.
        let Node::Split { id, b, .. } = panes.state.layout().clone() else {
            panic!("split");
        };
        panes.resize(id, 0.01);
        assert!(panes.area.width * panes.ratios.sidebar >= SIDEBAR_MIN_WIDTH - 1.0);
        let Node::Split { id: inner, .. } = *b else {
            panic!("inner split");
        };
        panes.resize(inner, 0.99);
        assert!(widths(&panes).2 >= PANEL_MIN_WIDTH - 0.5);
        // Bottom: graph and panel keep their heights.
        panes.set_position(DetailsPanelPosition::Bottom);
        let (_, graph) = panes.clamped();
        let height = panes.area.height - PANE_SPACING;
        assert!(height * graph >= BOTTOM_GRAPH_MIN_HEIGHT - 0.5);
        assert!(height * (1.0 - graph) >= BOTTOM_PANEL_MIN_HEIGHT - 0.5);
        // A larger window gives the stored ratio back; showing a hidden
        // panel again clamps it again.
        panes.set_position(DetailsPanelPosition::Right);
        panes.set_area(iced::Size::new(3000.0, 1600.0));
        panes.toggle_panel();
        panes.set_area(pane_area(min));
        panes.toggle_panel();
        assert!(widths(&panes).2 >= PANEL_MIN_WIDTH - 0.5);
    }
}
