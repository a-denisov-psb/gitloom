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

pub struct Panes {
    pub state: pane_grid::State<PaneKind>,
    pub ratios: PaneRatios,
    pub sidebar_visible: bool,
    pub panel_visible: bool,
    pub position: DetailsPanelPosition,
}

impl Panes {
    pub fn new(ratios: PaneRatios) -> Self {
        let mut panes = Self {
            state: pane_grid::State::new(PaneKind::Center).0,
            ratios: ratios.sanitized(),
            sidebar_visible: true,
            panel_visible: true,
            position: DetailsPanelPosition::Right,
        };
        panes.rebuild();
        panes
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

    /// Applies a drag on a split and remembers the new ratio.
    pub fn resize(&mut self, split: pane_grid::Split, ratio: f32) {
        self.state.resize(split, ratio);
        self.read_ratios();
    }

    /// The pane configuration for the visible panes and the position.
    pub fn configuration(&self) -> Configuration<PaneKind> {
        let pane = |kind| Box::new(Configuration::Pane(kind));
        let panel_axis = match self.position {
            DetailsPanelPosition::Right => Axis::Vertical,
            DetailsPanelPosition::Bottom => Axis::Horizontal,
        };
        let center_and_panel = || Configuration::Split {
            axis: panel_axis,
            ratio: self.ratios.panel_split(self.position),
            a: pane(PaneKind::Center),
            b: pane(PaneKind::Panel),
        };
        match (self.sidebar_visible, self.panel_visible) {
            (true, true) => Configuration::Split {
                axis: Axis::Vertical,
                ratio: self.ratios.sidebar,
                a: pane(PaneKind::Sidebar),
                b: Box::new(center_and_panel()),
            },
            (true, false) => Configuration::Split {
                axis: Axis::Vertical,
                ratio: self.ratios.sidebar,
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
}
