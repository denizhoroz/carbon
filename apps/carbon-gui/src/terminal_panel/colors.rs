//! Terminal colours from the app theme: the 16 ANSI colours use the theme's
//! base hues (so a theme change recolours terminals too); the 256-colour cube
//! and true colour are computed. Colours a program set (OSC 4/10/11) win.

use carbon_core::terminal::alacritty_terminal::term::color::Colors;
use carbon_core::terminal::alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};
use gpui_kit::component::Theme;
use gpui_kit::*;

/// How much dim text (SGR 2) and `Dim*` colours fade.
const DIM_OPACITY: f32 = 0.65;

#[derive(Clone)]
pub struct Palette {
    ansi: [Hsla; 16],
    pub foreground: Hsla,
    pub background: Hsla,
    pub cursor: Hsla,
    pub selection: Hsla,
    pub muted: Hsla,
    pub success: Hsla,
    pub error: Hsla,
}

impl Palette {
    pub fn new(theme: &Theme) -> Self {
        let c = &theme.colors;
        Self {
            ansi: [
                c.muted,
                c.red,
                c.green,
                c.yellow,
                c.blue,
                c.magenta,
                c.cyan,
                c.foreground,
                c.muted_foreground,
                c.red_light,
                c.green_light,
                c.yellow_light,
                c.blue_light,
                c.magenta_light,
                c.cyan_light,
                c.foreground,
            ],
            foreground: c.foreground,
            // The code editor's background (as the kit's editor picks it), so
            // a terminal beside or below the code reads as the same surface.
            background: theme
                .highlight_theme
                .style
                .editor_background
                .unwrap_or_else(|| theme.input_background()),
            cursor: c.caret,
            selection: c.selection,
            muted: c.muted_foreground,
            success: c.success,
            error: c.danger,
        }
    }

    pub fn resolve(&self, color: Color, overrides: &Colors) -> Hsla {
        match color {
            Color::Spec(rgb) => rgb_to_hsla(rgb),
            Color::Indexed(index) => match overrides[index as usize] {
                Some(rgb) => rgb_to_hsla(rgb),
                None => self.indexed(index),
            },
            Color::Named(name) => match overrides[name] {
                Some(rgb) => rgb_to_hsla(rgb),
                None => self.named(name),
            },
        }
    }

    fn named(&self, name: NamedColor) -> Hsla {
        let index = name as usize;
        if index < 16 {
            return self.ansi[index];
        }
        match name {
            NamedColor::Foreground | NamedColor::BrightForeground => self.foreground,
            NamedColor::Background => self.background,
            NamedColor::Cursor => self.cursor,
            NamedColor::DimForeground => self.foreground.opacity(DIM_OPACITY),
            // DimBlack .. DimWhite follow the normal colours, faded.
            dim => {
                let base = (dim as usize).saturating_sub(NamedColor::DimBlack as usize);
                self.ansi[base.min(7)].opacity(DIM_OPACITY)
            }
        }
    }

    fn indexed(&self, index: u8) -> Hsla {
        match index {
            0..=15 => self.ansi[index as usize],
            // 6x6x6 colour cube.
            16..=231 => {
                let i = index - 16;
                let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                rgb_to_hsla(Rgb {
                    r: level(i / 36),
                    g: level((i / 6) % 6),
                    b: level(i % 6),
                })
            }
            // Greyscale ramp.
            _ => {
                let v = 8 + (index - 232) * 10;
                rgb_to_hsla(Rgb { r: v, g: v, b: v })
            }
        }
    }

    /// Dim text: the colour faded.
    pub fn dim(color: Hsla) -> Hsla {
        color.opacity(DIM_OPACITY)
    }
}

fn rgb_to_hsla(rgb: Rgb) -> Hsla {
    gpui_kit::rgb(((rgb.r as u32) << 16) | ((rgb.g as u32) << 8) | rgb.b as u32).into()
}
