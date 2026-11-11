//! Icon set (ADR-025): Lucide icons (ISC license, `assets/icons/LICENSE`)
//! embedded at compile time and tinted with theme colors.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::LazyLock;

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{tree::Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::widget::svg::{self, Svg};
use iced::{mouse, Color, Element, Event, Length, Rectangle, Size, Theme, Vector};

use crate::theme::{palette, ThemePalette};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Undo,
    Redo,
    Pull,
    Push,
    Branch,
    Stash,
    Pop,
    Terminal,
    Settings,
    ChevronDown,
    ChevronRight,
    ChevronUp,
    Check,
    Close,
    Pencil,
    Grip,
    Refresh,
    /// Local branches (this computer).
    Local,
    /// Remote branches; also the remote marker on ref pills.
    Cloud,
    Tag,
    /// Submodules.
    Package,
    Info,
    Alert,
    Merge,
    FileAdded,
    FileModified,
    FileDeleted,
    FileRenamed,
    FileUntracked,
    FileConflicted,
    Eye,
    EyeOff,
    Solo,
    Plus,
    Minus,
    Search,
    History,
    /// Collapsed branch folder in the left panel.
    Folder,
    FolderOpen,
    /// Checked-out branch in the left panel.
    CheckSquare,
    /// External editor (M8-08).
    Editor,
    /// Toolbar toggles of the left and the commit panel (M11-14).
    PanelLeft,
    PanelRight,
}

impl Icon {
    pub const ALL: [Self; 43] = [
        Self::Undo,
        Self::Redo,
        Self::Pull,
        Self::Push,
        Self::Branch,
        Self::Stash,
        Self::Pop,
        Self::Terminal,
        Self::Settings,
        Self::ChevronDown,
        Self::ChevronRight,
        Self::ChevronUp,
        Self::Check,
        Self::Close,
        Self::Pencil,
        Self::Grip,
        Self::Refresh,
        Self::Local,
        Self::Cloud,
        Self::Tag,
        Self::Package,
        Self::Info,
        Self::Alert,
        Self::Merge,
        Self::FileAdded,
        Self::FileModified,
        Self::FileDeleted,
        Self::FileRenamed,
        Self::FileUntracked,
        Self::FileConflicted,
        Self::Eye,
        Self::EyeOff,
        Self::Solo,
        Self::Plus,
        Self::Minus,
        Self::Search,
        Self::History,
        Self::Folder,
        Self::FolderOpen,
        Self::CheckSquare,
        Self::Editor,
        Self::PanelLeft,
        Self::PanelRight,
    ];

    /// SVG source of the icon.
    fn bytes(self) -> &'static [u8] {
        macro_rules! lucide {
            ($name:literal) => {
                include_bytes!(concat!("../assets/icons/", $name, ".svg"))
            };
        }
        match self {
            Self::Undo => lucide!("undo-2"),
            Self::Redo => lucide!("redo-2"),
            Self::Pull => lucide!("arrow-down-to-line"),
            Self::Push => lucide!("arrow-up-from-line"),
            Self::Branch => lucide!("git-branch"),
            Self::Stash => lucide!("archive"),
            Self::Pop => lucide!("archive-restore"),
            Self::Terminal => lucide!("square-terminal"),
            Self::Settings => lucide!("settings"),
            Self::ChevronDown => lucide!("chevron-down"),
            Self::ChevronRight => lucide!("chevron-right"),
            Self::ChevronUp => lucide!("chevron-up"),
            Self::Check => lucide!("check"),
            Self::Close => lucide!("x"),
            Self::Pencil => lucide!("pencil"),
            Self::Grip => lucide!("grip-vertical"),
            Self::Refresh => lucide!("refresh-cw"),
            Self::Local => lucide!("monitor"),
            Self::Cloud => lucide!("cloud"),
            Self::Tag => lucide!("tag"),
            Self::Package => lucide!("package"),
            Self::Info => lucide!("info"),
            Self::Alert => lucide!("circle-alert"),
            Self::Merge => lucide!("git-merge"),
            Self::FileAdded => lucide!("square-plus"),
            Self::FileModified => lucide!("square-pen"),
            Self::FileDeleted => lucide!("square-minus"),
            Self::FileRenamed => lucide!("square-arrow-right"),
            Self::FileUntracked => lucide!("square-dashed"),
            Self::FileConflicted => lucide!("triangle-alert"),
            Self::Eye => lucide!("eye"),
            Self::EyeOff => lucide!("eye-off"),
            Self::Solo => lucide!("locate-fixed"),
            Self::Plus => lucide!("plus"),
            Self::Minus => lucide!("minus"),
            Self::Search => lucide!("search"),
            Self::History => lucide!("history"),
            Self::Folder => lucide!("folder"),
            Self::FolderOpen => lucide!("folder-open"),
            Self::CheckSquare => lucide!("square-check"),
            Self::Editor => lucide!("code-xml"),
            Self::PanelLeft => lucide!("panel-left"),
            Self::PanelRight => lucide!("panel-right"),
        }
    }

    fn handle(self) -> svg::Handle {
        static HANDLES: LazyLock<Vec<svg::Handle>> = LazyLock::new(|| {
            Icon::ALL
                .iter()
                .map(|icon| svg::Handle::from_memory(icon.bytes()))
                .collect()
        });
        HANDLES[self as usize].clone()
    }
}

/// Physical pixels per logical pixel: zoom × window scale factor (M9-05).
/// Stored as `f32` bits; set by the app before each view.
static SCALE: AtomicU32 = AtomicU32::new(0x3f80_0000); // 1.0

/// Sets the scale icons are snapped to (zoom × window scale factor).
pub fn set_scale(scale: f32) {
    if scale.is_finite() && scale > 0.0 {
        SCALE.store(scale.to_bits(), Ordering::Relaxed);
    }
}

fn scale() -> f32 {
    f32::from_bits(SCALE.load(Ordering::Relaxed))
}

/// `size` rounded so that it covers a whole number of physical pixels
/// (at least one) at `scale`.
pub fn snap_size(size: f32, scale: f32) -> f32 {
    (size * scale).round().max(1.0) / scale
}

/// Logical offset that moves `position` onto the physical pixel grid.
pub fn snap_offset(position: f32, scale: f32) -> f32 {
    let physical = position * scale;
    (physical.round() - physical) / scale
}

/// An icon drawn on whole physical pixels, so it stays sharp at fractional
/// zoom levels and window scale factors (M9-05): its size is rounded by
/// [`snap_size`], its position moved by [`snap_offset`] when drawn.
pub struct Sharp<'a> {
    svg: Svg<'a, Theme>,
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for Sharp<'_>
where
    Renderer: iced::advanced::svg::Renderer,
{
    fn size(&self) -> Size<Length> {
        <Svg<'_, Theme> as Widget<Message, Theme, Renderer>>::size(&self.svg)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        <Svg<'_, Theme> as Widget<Message, Theme, Renderer>>::layout(
            &mut self.svg,
            tree,
            renderer,
            limits,
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.svg.update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let scale = scale();
        let bounds = layout.bounds();
        let offset = Vector::new(snap_offset(bounds.x, scale), snap_offset(bounds.y, scale));
        renderer.with_translation(offset, |renderer| {
            <Svg<'_, Theme> as Widget<Message, Theme, Renderer>>::draw(
                &self.svg, tree, renderer, theme, style, layout, cursor, viewport,
            );
        });
    }
}

impl<'a, Message, Renderer> From<Sharp<'a>> for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Renderer: iced::advanced::svg::Renderer + 'a,
{
    fn from(icon: Sharp<'a>) -> Self {
        Element::new(icon)
    }
}

/// Icon in the normal text color.
pub fn icon<'a>(icon: Icon, size: f32) -> Sharp<'a> {
    tinted(icon, size, |p| p.text)
}

/// Icon in a palette color.
pub fn tinted<'a>(icon: Icon, size: f32, pick: fn(&ThemePalette) -> Color) -> Sharp<'a> {
    colored(icon, size, move |theme| pick(palette(theme)))
}

/// Icon in a color derived from the theme.
pub fn colored<'a>(icon: Icon, size: f32, color: impl Fn(&Theme) -> Color + 'a) -> Sharp<'a> {
    let size = snap_size(size, scale());
    Sharp {
        svg: iced::widget::svg(icon.handle())
            .width(size)
            .height(size)
            .style(move |theme, _status| svg::Style {
                color: Some(color(theme)),
            }),
    }
}

/// Icon of a toolbar or header button: dimmed while the button is disabled
/// (buttons do not tint their content).
pub fn button_icon<'a>(icon: Icon, size: f32, enabled: bool) -> Sharp<'a> {
    if enabled {
        tinted(icon, size, |p| p.text)
    } else {
        tinted(icon, size, |p| p.text_muted.scale_alpha(0.5))
    }
}

/// Icon for a file change kind, in the kind's color.
pub fn change_icon<'a>(kind: git_core::ChangeKind, size: f32) -> Sharp<'a> {
    use git_core::ChangeKind;
    let icon = match kind {
        ChangeKind::Added => Icon::FileAdded,
        ChangeKind::Modified => Icon::FileModified,
        ChangeKind::Deleted => Icon::FileDeleted,
        ChangeKind::Renamed => Icon::FileRenamed,
        ChangeKind::Untracked => Icon::FileUntracked,
        ChangeKind::Conflicted => Icon::FileConflicted,
    };
    colored(icon, size, move |theme| {
        crate::theme::change_color(palette(theme), kind)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_listed_once_in_order() {
        for (index, icon) in Icon::ALL.iter().enumerate() {
            assert_eq!(*icon as usize, index, "{icon:?}");
        }
    }

    #[test]
    fn icons_are_lucide_svgs_tinted_by_current_color() {
        for icon in Icon::ALL {
            let source = std::str::from_utf8(icon.bytes()).unwrap();
            assert!(source.contains("<svg"), "{icon:?}");
            assert!(source.contains("lucide"), "{icon:?}");
            assert!(source.contains("stroke=\"currentColor\""), "{icon:?}");
        }
    }

    #[test]
    fn sizes_cover_whole_physical_pixels() {
        for scale in [1.0_f32, 1.1, 1.25, 1.5, 2.0, 2.2, 2.5] {
            for size in [10.0_f32, 12.0, 13.0, 14.0, 15.0, 16.0] {
                let snapped = snap_size(size, scale);
                let physical = snapped * scale;
                assert!(
                    (physical - physical.round()).abs() < 1e-3,
                    "{size} @ {scale}"
                );
                assert!(
                    (snapped - size).abs() <= 0.5 / scale + 1e-4,
                    "{size} @ {scale}"
                );
            }
        }
        assert_eq!(snap_size(14.0, 1.0), 14.0);
        // 14 × 1.25 = 17.5 → 18 physical pixels.
        assert!((snap_size(14.0, 1.25) - 14.4).abs() < 1e-4);
        assert!(snap_size(0.1, 1.0) >= 1.0, "at least one pixel");
    }

    #[test]
    fn offsets_move_positions_onto_the_pixel_grid() {
        for scale in [1.0_f32, 1.1, 1.25, 1.5] {
            for position in [0.0_f32, 3.3, 10.5, 17.75, 101.2] {
                let moved = (position + snap_offset(position, scale)) * scale;
                assert!((moved - moved.round()).abs() < 1e-3, "{position} @ {scale}");
                assert!(snap_offset(position, scale).abs() <= 0.5 / scale + 1e-4);
            }
        }
        assert_eq!(snap_offset(12.0, 1.0), 0.0);
    }

    #[test]
    fn license_is_vendored() {
        let license = include_str!("../assets/icons/LICENSE");
        assert!(license.starts_with("ISC License"));
    }
}
