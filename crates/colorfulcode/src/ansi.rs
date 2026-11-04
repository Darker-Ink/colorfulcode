//! ANSI escape code rendering for terminals.

use crate::{Color, Style, Theme, Token};
use std::fmt::Write;

/// Which color escape codes to emit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// 24 bit colors. Supported by nearly every modern terminal.
    #[default]
    TrueColor,
    /// The 256 color palette, for older terminals.
    Ansi256,
}

impl ColorMode {
    /// Picks a mode from the `COLORTERM` environment variable.
    pub fn detect() -> ColorMode {
        match std::env::var("COLORTERM") {
            Ok(v) if v == "truecolor" || v == "24bit" => ColorMode::TrueColor,
            _ if std::env::var("TERM_PROGRAM").is_ok_and(|p| p == "Apple_Terminal") => ColorMode::Ansi256,
            _ => ColorMode::TrueColor,
        }
    }
}

/// Renders tokens with ANSI colors. Styles are reset before every line
/// break so the output can be safely split into lines.
pub fn render(code: &str, tokens: &[Token], theme: &Theme, mode: ColorMode) -> String {
    let mut out = String::with_capacity(code.len() * 3);
    for t in tokens {
        let style = theme.style(t.kind);
        let mut first = true;
        for piece in code[t.range()].split('\n') {
            if !first {
                out.push('\n');
            }
            first = false;
            paint(&mut out, piece, style, mode);
        }
    }
    out
}

/// Appends `text` drawn with `style`, followed by a reset.
pub fn paint(out: &mut String, text: &str, style: Style, mode: ColorMode) {
    if text.is_empty() {
        return;
    }
    out.push_str("\x1b[");
    if style.bold {
        out.push_str("1;");
    }
    if style.italic {
        out.push_str("3;");
    }
    if style.underline {
        out.push_str("4;");
    }
    push_fg(out, style.color, mode);
    out.push('m');
    out.push_str(text);
    out.push_str("\x1b[0m");
}

/// Appends the parameters selecting `color` as foreground (without `ESC[` or `m`).
pub fn push_fg(out: &mut String, color: Color, mode: ColorMode) {
    match mode {
        ColorMode::TrueColor => {
            let _ = write!(out, "38;2;{};{};{}", color.r, color.g, color.b);
        }
        ColorMode::Ansi256 => {
            let _ = write!(out, "38;5;{}", to_256(color));
        }
    }
}

/// Appends the parameters selecting `color` as background (without `ESC[` or `m`).
pub fn push_bg(out: &mut String, color: Color, mode: ColorMode) {
    match mode {
        ColorMode::TrueColor => {
            let _ = write!(out, "48;2;{};{};{}", color.r, color.g, color.b);
        }
        ColorMode::Ansi256 => {
            let _ = write!(out, "48;5;{}", to_256(color));
        }
    }
}

/// Nearest color in the xterm 256 color palette.
pub fn to_256(c: Color) -> u8 {
    const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let cube = |v: u8| -> usize {
        STEPS.iter().enumerate().min_by_key(|(_, &s)| (s as i32 - v as i32).abs()).map_or(0, |(i, _)| i)
    };
    let (r, g, b) = (cube(c.r), cube(c.g), cube(c.b));
    let cube_color = (STEPS[r], STEPS[g], STEPS[b]);
    let avg = (c.r as u32 + c.g as u32 + c.b as u32) / 3;
    let gray_index = if avg > 238 { 23 } else { avg.saturating_sub(3) / 10 };
    let gray = (8 + gray_index * 10) as u8;
    let dist = |(r2, g2, b2): (u8, u8, u8)| {
        let d = |a: u8, b: u8| (a as i32 - b as i32).pow(2);
        d(c.r, r2) + d(c.g, g2) + d(c.b, b2)
    };
    if dist((gray, gray, gray)) < dist(cube_color) {
        232 + gray_index as u8
    } else {
        16 + (36 * r + 6 * g + b) as u8
    }
}
