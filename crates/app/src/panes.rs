//! Pane layout: sidebar | center | detail panel, with hideable side panels.

use iced::widget::pane_grid::{self, Axis, Configuration, Node};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    Sidebar,
    Center,
    Panel,
}

/// Split ratios, persisted in the session.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PaneRatios {
    /// Sidebar width as fraction of the window.
    pub sidebar: f32,
    /// Center width as fraction of center + detail panel.
    pub center: f32,
}

impl Default for PaneRatios {
    fn default() -> Self {
        Self {
            sidebar: 0.18,
            center: 0.72,
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
        }
    }
}

pub struct Panes {
    pub state: pane_grid::State<PaneKind>,
    pub ratios: PaneRatios,
    pub sidebar_visible: bool,
    pub panel_visible: bool,
}

impl Panes {
    pub fn new(ratios: PaneRatios) -> Self {
        let mut panes = Self {
            state: pane_grid::State::new(PaneKind::Center).0,
            ratios: ratios.sanitized(),
            sidebar_visible: true,
            panel_visible: true,
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

    /// Applies a drag on a split and remembers the new ratio.
    pub fn resize(&mut self, split: pane_grid::Split, ratio: f32) {
        self.state.resize(split, ratio);
        self.read_ratios();
    }

    fn rebuild(&mut self) {
        let pane = |kind| Box::new(Configuration::Pane(kind));
        let config = match (self.sidebar_visible, self.panel_visible) {
            (true, true) => Configuration::Split {
                axis: Axis::Vertical,
                ratio: self.ratios.sidebar,
                a: pane(PaneKind::Sidebar),
                b: Box::new(Configuration::Split {
                    axis: Axis::Vertical,
                    ratio: self.ratios.center,
                    a: pane(PaneKind::Center),
                    b: pane(PaneKind::Panel),
                }),
            },
            (true, false) => Configuration::Split {
                axis: Axis::Vertical,
                ratio: self.ratios.sidebar,
                a: pane(PaneKind::Sidebar),
                b: pane(PaneKind::Center),
            },
            (false, true) => Configuration::Split {
                axis: Axis::Vertical,
                ratio: self.ratios.center,
                a: pane(PaneKind::Center),
                b: pane(PaneKind::Panel),
            },
            (false, false) => Configuration::Pane(PaneKind::Center),
        };
        self.state = pane_grid::State::with_configuration(config);
    }

    /// Reads the ratios of the visible splits back into `ratios`.
    fn read_ratios(&mut self) {
        let Node::Split { ratio, b, .. } = self.state.layout() else {
            return;
        };
        match (self.sidebar_visible, self.panel_visible) {
            (true, true) => {
                self.ratios.sidebar = *ratio;
                if let Node::Split { ratio: center, .. } = b.as_ref() {
                    self.ratios.center = *center;
                }
            }
            (true, false) => self.ratios.sidebar = *ratio,
            (false, true) => self.ratios.center = *ratio,
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
        });
        assert_eq!(panes.ratios.sidebar, PaneRatios::default().sidebar);
        assert_eq!(panes.ratios.center, 0.95);
    }
}
