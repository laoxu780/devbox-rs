//! Visual design tokens, ported from the original web UI.

use gpui::{Rgba, Window, WindowAppearance, rgb, rgba};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "跟随系统",
            Self::Light => "浅色",
            Self::Dark => "深色",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }

    pub fn resolve(self, window: &Window) -> Theme {
        let dark = match self {
            Self::Light => false,
            Self::Dark => true,
            Self::System => matches!(
                window.appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
        };
        Theme::new(dark)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub dark: bool,
    pub canvas: u32,
    pub card: u32,
    pub card2: u32,
    pub inset: u32,
    pub line: u32,
    pub line_strong: u32,
    pub text: u32,
    pub muted: u32,
    pub faint: u32,
    pub accent: u32,
    pub accent_strong: u32,
    pub accent_tint: u32,
    pub accent_ghost: u32,
    pub ok: u32,
    pub danger: u32,
    pub danger_tint: u32,
    pub selection: u32,
}

impl Theme {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                dark,
                canvas: 0x17181b,
                card: 0x212227,
                card2: 0x26272d,
                inset: 0x1b1c20,
                line: 0x303138,
                line_strong: 0x3a3b43,
                text: 0xe8e9ed,
                muted: 0xa2a4ae,
                faint: 0x71737d,
                accent: 0x8f80f2,
                accent_strong: 0xa497f5,
                accent_tint: 0x322d4a,
                accent_ghost: 0x8f80f21a,
                ok: 0x5bb98b,
                danger: 0xe58080,
                danger_tint: 0xe580801f,
                selection: 0x8f80f24d,
            }
        } else {
            Self {
                dark,
                canvas: 0xf4f5f7,
                card: 0xffffff,
                card2: 0xfafafc,
                inset: 0xf4f5f8,
                line: 0xe6e7ec,
                line_strong: 0xd8d9e0,
                text: 0x1b1c20,
                muted: 0x62646e,
                faint: 0x9a9ca6,
                accent: 0x6d5bd0,
                accent_strong: 0x5a49c0,
                accent_tint: 0xeeebfb,
                accent_ghost: 0x6d5bd017,
                ok: 0x2f9e63,
                danger: 0xd64545,
                danger_tint: 0xfdeeee,
                selection: 0x6d5bd040,
            }
        }
    }

    pub fn canvas(&self) -> Rgba {
        rgb(self.canvas)
    }
    pub fn card(&self) -> Rgba {
        rgb(self.card)
    }
    pub fn card2(&self) -> Rgba {
        rgb(self.card2)
    }
    pub fn inset(&self) -> Rgba {
        rgb(self.inset)
    }
    pub fn line(&self) -> Rgba {
        rgb(self.line)
    }
    pub fn line_strong(&self) -> Rgba {
        rgb(self.line_strong)
    }
    pub fn text(&self) -> Rgba {
        rgb(self.text)
    }
    pub fn muted(&self) -> Rgba {
        rgb(self.muted)
    }
    pub fn faint(&self) -> Rgba {
        rgb(self.faint)
    }
    pub fn accent(&self) -> Rgba {
        rgb(self.accent)
    }
    pub fn accent_strong(&self) -> Rgba {
        rgb(self.accent_strong)
    }
    pub fn accent_tint(&self) -> Rgba {
        rgb(self.accent_tint)
    }
    pub fn accent_ghost(&self) -> Rgba {
        rgba(self.accent_ghost)
    }
    pub fn ok(&self) -> Rgba {
        rgb(self.ok)
    }
    pub fn danger(&self) -> Rgba {
        rgb(self.danger)
    }
    pub fn danger_tint(&self) -> Rgba {
        rgba(self.danger_tint)
    }
    pub fn selection(&self) -> Rgba {
        rgba(self.selection)
    }
}
