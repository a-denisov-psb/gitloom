//! Icon set (ADR-025): Lucide icons (ISC license, `assets/icons/LICENSE`)
//! embedded at compile time and tinted with theme colors.

use std::sync::LazyLock;

use iced::widget::svg::{self, Svg};
use iced::{Color, Theme};

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
}

impl Icon {
    pub const ALL: [Self; 37] = [
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

/// Icon in the normal text color.
pub fn icon<'a>(icon: Icon, size: f32) -> Svg<'a, Theme> {
    tinted(icon, size, |p| p.text)
}

/// Icon in a palette color.
pub fn tinted<'a>(icon: Icon, size: f32, pick: fn(&ThemePalette) -> Color) -> Svg<'a, Theme> {
    colored(icon, size, move |theme| pick(palette(theme)))
}

/// Icon in a color derived from the theme.
pub fn colored<'a>(icon: Icon, size: f32, color: impl Fn(&Theme) -> Color + 'a) -> Svg<'a, Theme> {
    iced::widget::svg(icon.handle())
        .width(size)
        .height(size)
        .style(move |theme, _status| svg::Style {
            color: Some(color(theme)),
        })
}

/// Icon of a toolbar or header button: dimmed while the button is disabled
/// (buttons do not tint their content).
pub fn button_icon<'a>(icon: Icon, size: f32, enabled: bool) -> Svg<'a, Theme> {
    if enabled {
        tinted(icon, size, |p| p.text)
    } else {
        tinted(icon, size, |p| p.text_muted.scale_alpha(0.5))
    }
}

/// Icon for a file change kind, in the kind's color.
pub fn change_icon<'a>(kind: git_core::ChangeKind, size: f32) -> Svg<'a, Theme> {
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
    fn license_is_vendored() {
        let license = include_str!("../assets/icons/LICENSE");
        assert!(license.starts_with("ISC License"));
    }
}
