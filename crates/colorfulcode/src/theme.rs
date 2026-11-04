//! Color themes.

use crate::Kind;
use std::fmt::Write;

/// An sRGB color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b }
    }

    /// Parses `#rrggbb` (the `#` is optional). Panics on invalid input when
    /// used in a const context, returns black otherwise.
    pub const fn hex(s: &str) -> Color {
        let b = s.as_bytes();
        let o = if !b.is_empty() && b[0] == b'#' { 1 } else { 0 };
        if b.len() != o + 6 {
            return Color::rgb(0, 0, 0);
        }
        Color::rgb(hex_byte(b[o], b[o + 1]), hex_byte(b[o + 2], b[o + 3]), hex_byte(b[o + 4], b[o + 5]))
    }

    /// Packs the color as `0xRRGGBB`.
    pub const fn to_u32(self) -> u32 {
        (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

const fn hex_digit(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

const fn hex_byte(hi: u8, lo: u8) -> u8 {
    hex_digit(hi) * 16 + hex_digit(lo)
}

/// How a token kind is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Style {
    pub color: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

/// A color theme: a background, a foreground and a [`Style`] for every [`Kind`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub name: &'static str,
    pub dark: bool,
    pub background: Color,
    pub foreground: Color,
    styles: [Style; Kind::COUNT],
}

const B: u8 = 1;
const I: u8 = 2;
const U: u8 = 4;

impl Theme {
    /// Builds a theme. Kinds missing from `rules` inherit from [`Kind::fallback`].
    const fn build(
        name: &'static str,
        dark: bool,
        background: &str,
        foreground: &str,
        rules: &[(Kind, &str, u8)],
    ) -> Theme {
        let fg = Color::hex(foreground);
        let plain = Style { color: fg, bold: false, italic: false, underline: false };
        let mut set = [false; Kind::COUNT];
        let mut styles = [plain; Kind::COUNT];
        let mut i = 0;
        while i < rules.len() {
            let (kind, color, flags) = rules[i];
            styles[kind as usize] = Style {
                color: Color::hex(color),
                bold: flags & B != 0,
                italic: flags & I != 0,
                underline: flags & U != 0,
            };
            set[kind as usize] = true;
            i += 1;
        }
        // Resolve fallbacks for kinds the theme did not mention.
        let mut k = 0;
        while k < Kind::COUNT {
            if !set[k] {
                let mut cur = Kind::ALL[k];
                while let Some(parent) = cur.fallback() {
                    if set[parent as usize] {
                        styles[k] = styles[parent as usize];
                        break;
                    }
                    cur = parent;
                }
                // Text decorations that should survive any fallback.
                match Kind::ALL[k] {
                    Kind::Heading | Kind::Strong => styles[k].bold = true,
                    Kind::Emphasis => styles[k].italic = true,
                    Kind::Link => styles[k].underline = true,
                    _ => {}
                }
            }
            k += 1;
        }
        Theme { name, dark, background: Color::hex(background), foreground: fg, styles }
    }

    /// Style for a kind.
    #[inline]
    pub fn style(&self, kind: Kind) -> Style {
        self.styles[kind as usize]
    }

    /// Every built in theme.
    pub fn all() -> &'static [Theme] {
        THEMES
    }

    /// Finds a built in theme by name (case insensitive).
    pub fn find(name: &str) -> Option<&'static Theme> {
        THEMES.iter().find(|t| t.name.eq_ignore_ascii_case(name.trim()))
    }

    /// CSS for the class based HTML output, scoped to `selector`
    /// (for example `".cc"`). Kinds styled like plain text are skipped.
    pub fn to_css(&self, selector: &str) -> String {
        let mut css = String::new();
        let _ = writeln!(css, "{selector}{{background:{};color:{}}}", self.background, self.foreground);
        for kind in Kind::ALL.iter().skip(1) {
            let s = self.style(*kind);
            let _ = write!(css, "{selector} .{}{{color:{}", kind.class(), s.color);
            if s.bold {
                css.push_str(";font-weight:bold");
            }
            if s.italic {
                css.push_str(";font-style:italic");
            }
            if s.underline {
                css.push_str(";text-decoration:underline");
            }
            css.push_str("}\n");
        }
        css
    }
}

impl Default for Theme {
    fn default() -> Self {
        THEMES[0].clone()
    }
}

use builtin::THEMES;

mod builtin {
    use super::{Theme, B, I, U};
    use crate::Kind::*;

    pub static THEMES: &[Theme] = &[
        Theme::build(
            "colorful",
            true,
            "#12131c",
            "#e4e6f1",
            &[
                (Comment, "#6b7194", I),
                (Keyword, "#ff6ac1", 0),
                (String, "#8ee59c", 0),
                (Escape, "#ffd166", 0),
                (Number, "#ffb86c", 0),
                (Constant, "#ff9e64", 0),
                (Function, "#61d4ff", 0),
                (Type, "#ffd166", 0),
                (Variable, "#f2a7ff", 0),
                (Property, "#9fb4ff", 0),
                (Builtin, "#4fe0c4", 0),
                (Operator, "#ff8fa3", 0),
                (Punctuation, "#a3a8c3", 0),
                (Tag, "#ff6ac1", 0),
                (Attribute, "#c792ea", 0),
                (Meta, "#c792ea", 0),
                (Regex, "#f78c6c", 0),
                (Heading, "#61d4ff", B),
                (Link, "#4fe0c4", U),
                (Inserted, "#8ee59c", 0),
                (Deleted, "#ff6b6b", 0),
            ],
        ),
        Theme::build(
            "colorful-light",
            false,
            "#fbfbfd",
            "#24273a",
            &[
                (Comment, "#8a8fa8", I),
                (Keyword, "#d6246e", 0),
                (String, "#2e8b3a", 0),
                (Escape, "#b8860b", 0),
                (Number, "#c2570c", 0),
                (Constant, "#c2570c", 0),
                (Function, "#1f6feb", 0),
                (Type, "#a15c00", 0),
                (Variable, "#9c27b0", 0),
                (Property, "#6639ba", 0),
                (Builtin, "#0e7c86", 0),
                (Operator, "#d6246e", 0),
                (Punctuation, "#5c6075", 0),
                (Tag, "#d6246e", 0),
                (Attribute, "#6639ba", 0),
                (Meta, "#6639ba", 0),
                (Regex, "#b3401b", 0),
                (Heading, "#1f6feb", B),
                (Link, "#0e7c86", U),
                (Inserted, "#2e8b3a", 0),
                (Deleted, "#d1242f", 0),
            ],
        ),
        Theme::build(
            "github-dark",
            true,
            "#0d1117",
            "#e6edf3",
            &[
                (Comment, "#8b949e", 0),
                (Keyword, "#ff7b72", 0),
                (String, "#a5d6ff", 0),
                (Escape, "#79c0ff", 0),
                (Number, "#79c0ff", 0),
                (Constant, "#79c0ff", 0),
                (Function, "#d2a8ff", 0),
                (Type, "#ffa657", 0),
                (Variable, "#ffa657", 0),
                (Property, "#79c0ff", 0),
                (Builtin, "#79c0ff", 0),
                (Operator, "#ff7b72", 0),
                (Punctuation, "#e6edf3", 0),
                (Tag, "#7ee787", 0),
                (Attribute, "#79c0ff", 0),
                (Meta, "#d2a8ff", 0),
                (Regex, "#a5d6ff", 0),
                (Heading, "#1f6feb", B),
                (Link, "#a5d6ff", U),
                (Inserted, "#aff5b4", 0),
                (Deleted, "#ffdcd7", 0),
            ],
        ),
        Theme::build(
            "github-light",
            false,
            "#ffffff",
            "#1f2328",
            &[
                (Comment, "#6e7781", 0),
                (Keyword, "#cf222e", 0),
                (String, "#0a3069", 0),
                (Escape, "#0550ae", 0),
                (Number, "#0550ae", 0),
                (Constant, "#0550ae", 0),
                (Function, "#8250df", 0),
                (Type, "#953800", 0),
                (Variable, "#953800", 0),
                (Property, "#0550ae", 0),
                (Builtin, "#0550ae", 0),
                (Operator, "#cf222e", 0),
                (Punctuation, "#1f2328", 0),
                (Tag, "#116329", 0),
                (Attribute, "#0550ae", 0),
                (Meta, "#8250df", 0),
                (Regex, "#0a3069", 0),
                (Heading, "#0550ae", B),
                (Link, "#0a3069", U),
                (Inserted, "#116329", 0),
                (Deleted, "#82071e", 0),
            ],
        ),
        Theme::build(
            "dracula",
            true,
            "#282a36",
            "#f8f8f2",
            &[
                (Comment, "#6272a4", 0),
                (Keyword, "#ff79c6", 0),
                (String, "#f1fa8c", 0),
                (Escape, "#ff79c6", 0),
                (Number, "#bd93f9", 0),
                (Constant, "#bd93f9", 0),
                (Function, "#50fa7b", 0),
                (Type, "#8be9fd", I),
                (Variable, "#ffb86c", I),
                (Property, "#f8f8f2", 0),
                (Builtin, "#8be9fd", 0),
                (Operator, "#ff79c6", 0),
                (Punctuation, "#f8f8f2", 0),
                (Tag, "#ff79c6", 0),
                (Attribute, "#50fa7b", I),
                (Meta, "#50fa7b", I),
                (Regex, "#ff5555", 0),
                (Heading, "#bd93f9", B),
                (Link, "#8be9fd", U),
                (Inserted, "#50fa7b", 0),
                (Deleted, "#ff5555", 0),
            ],
        ),
        Theme::build(
            "nord",
            true,
            "#2e3440",
            "#d8dee9",
            &[
                (Comment, "#616e88", I),
                (Keyword, "#81a1c1", 0),
                (String, "#a3be8c", 0),
                (Escape, "#ebcb8b", 0),
                (Number, "#b48ead", 0),
                (Constant, "#81a1c1", 0),
                (Function, "#88c0d0", 0),
                (Type, "#8fbcbb", 0),
                (Variable, "#d8dee9", 0),
                (Property, "#d8dee9", 0),
                (Builtin, "#88c0d0", 0),
                (Operator, "#81a1c1", 0),
                (Punctuation, "#eceff4", 0),
                (Tag, "#81a1c1", 0),
                (Attribute, "#8fbcbb", 0),
                (Meta, "#d08770", 0),
                (Regex, "#ebcb8b", 0),
                (Heading, "#88c0d0", B),
                (Link, "#88c0d0", U),
                (Inserted, "#a3be8c", 0),
                (Deleted, "#bf616a", 0),
            ],
        ),
        Theme::build(
            "one-dark",
            true,
            "#282c34",
            "#abb2bf",
            &[
                (Comment, "#5c6370", I),
                (Keyword, "#c678dd", 0),
                (String, "#98c379", 0),
                (Escape, "#56b6c2", 0),
                (Number, "#d19a66", 0),
                (Constant, "#d19a66", 0),
                (Function, "#61afef", 0),
                (Type, "#e5c07b", 0),
                (Variable, "#e06c75", 0),
                (Property, "#e06c75", 0),
                (Builtin, "#56b6c2", 0),
                (Operator, "#56b6c2", 0),
                (Punctuation, "#abb2bf", 0),
                (Tag, "#e06c75", 0),
                (Attribute, "#d19a66", 0),
                (Meta, "#61afef", 0),
                (Regex, "#56b6c2", 0),
                (Heading, "#e06c75", B),
                (Link, "#61afef", U),
                (Inserted, "#98c379", 0),
                (Deleted, "#e06c75", 0),
            ],
        ),
        Theme::build(
            "monokai",
            true,
            "#272822",
            "#f8f8f2",
            &[
                (Comment, "#75715e", 0),
                (Keyword, "#f92672", 0),
                (String, "#e6db74", 0),
                (Escape, "#ae81ff", 0),
                (Number, "#ae81ff", 0),
                (Constant, "#ae81ff", 0),
                (Function, "#a6e22e", 0),
                (Type, "#66d9ef", I),
                (Variable, "#fd971f", 0),
                (Property, "#f8f8f2", 0),
                (Builtin, "#66d9ef", 0),
                (Operator, "#f92672", 0),
                (Punctuation, "#f8f8f2", 0),
                (Tag, "#f92672", 0),
                (Attribute, "#a6e22e", 0),
                (Meta, "#a6e22e", 0),
                (Regex, "#e6db74", 0),
                (Heading, "#a6e22e", B),
                (Link, "#66d9ef", U),
                (Inserted, "#a6e22e", 0),
                (Deleted, "#f92672", 0),
            ],
        ),
        Theme::build(
            "catppuccin-mocha",
            true,
            "#1e1e2e",
            "#cdd6f4",
            &[
                (Comment, "#9399b2", I),
                (Keyword, "#cba6f7", 0),
                (String, "#a6e3a1", 0),
                (Escape, "#f5c2e7", 0),
                (Number, "#fab387", 0),
                (Constant, "#fab387", 0),
                (Function, "#89b4fa", 0),
                (Type, "#f9e2af", 0),
                (Variable, "#f38ba8", 0),
                (Property, "#b4befe", 0),
                (Builtin, "#89dceb", 0),
                (Operator, "#89dceb", 0),
                (Punctuation, "#9399b2", 0),
                (Tag, "#89b4fa", 0),
                (Attribute, "#f9e2af", 0),
                (Meta, "#fab387", 0),
                (Regex, "#f5c2e7", 0),
                (Heading, "#f38ba8", B),
                (Link, "#89b4fa", U),
                (Inserted, "#a6e3a1", 0),
                (Deleted, "#f38ba8", 0),
            ],
        ),
        Theme::build(
            "catppuccin-latte",
            false,
            "#eff1f5",
            "#4c4f69",
            &[
                (Comment, "#7c7f93", I),
                (Keyword, "#8839ef", 0),
                (String, "#40a02b", 0),
                (Escape, "#ea76cb", 0),
                (Number, "#fe640b", 0),
                (Constant, "#fe640b", 0),
                (Function, "#1e66f5", 0),
                (Type, "#df8e1d", 0),
                (Variable, "#d20f39", 0),
                (Property, "#7287fd", 0),
                (Builtin, "#04a5e5", 0),
                (Operator, "#04a5e5", 0),
                (Punctuation, "#7c7f93", 0),
                (Tag, "#1e66f5", 0),
                (Attribute, "#df8e1d", 0),
                (Meta, "#fe640b", 0),
                (Regex, "#ea76cb", 0),
                (Heading, "#d20f39", B),
                (Link, "#1e66f5", U),
                (Inserted, "#40a02b", 0),
                (Deleted, "#d20f39", 0),
            ],
        ),
        Theme::build(
            "tokyo-night",
            true,
            "#1a1b26",
            "#c0caf5",
            &[
                (Comment, "#565f89", I),
                (Keyword, "#bb9af7", 0),
                (String, "#9ece6a", 0),
                (Escape, "#89ddff", 0),
                (Number, "#ff9e64", 0),
                (Constant, "#ff9e64", 0),
                (Function, "#7aa2f7", 0),
                (Type, "#2ac3de", 0),
                (Variable, "#e0af68", 0),
                (Property, "#73daca", 0),
                (Builtin, "#7dcfff", 0),
                (Operator, "#89ddff", 0),
                (Punctuation, "#9abdf5", 0),
                (Tag, "#f7768e", 0),
                (Attribute, "#bb9af7", 0),
                (Meta, "#e0af68", 0),
                (Regex, "#b4f9f8", 0),
                (Heading, "#89ddff", B),
                (Link, "#7aa2f7", U),
                (Inserted, "#9ece6a", 0),
                (Deleted, "#f7768e", 0),
            ],
        ),
        Theme::build(
            "gruvbox-dark",
            true,
            "#282828",
            "#ebdbb2",
            &[
                (Comment, "#928374", I),
                (Keyword, "#fb4934", 0),
                (String, "#b8bb26", 0),
                (Escape, "#fe8019", 0),
                (Number, "#d3869b", 0),
                (Constant, "#d3869b", 0),
                (Function, "#8ec07c", 0),
                (Type, "#fabd2f", 0),
                (Variable, "#83a598", 0),
                (Property, "#83a598", 0),
                (Builtin, "#fe8019", 0),
                (Operator, "#fe8019", 0),
                (Punctuation, "#a89984", 0),
                (Tag, "#fb4934", 0),
                (Attribute, "#8ec07c", 0),
                (Meta, "#fe8019", 0),
                (Regex, "#b8bb26", 0),
                (Heading, "#fabd2f", B),
                (Link, "#83a598", U),
                (Inserted, "#b8bb26", 0),
                (Deleted, "#fb4934", 0),
            ],
        ),
        Theme::build(
            "solarized-dark",
            true,
            "#002b36",
            "#839496",
            &[
                (Comment, "#586e75", I),
                (Keyword, "#859900", 0),
                (String, "#2aa198", 0),
                (Escape, "#dc322f", 0),
                (Number, "#d33682", 0),
                (Constant, "#cb4b16", 0),
                (Function, "#268bd2", 0),
                (Type, "#b58900", 0),
                (Variable, "#268bd2", 0),
                (Property, "#268bd2", 0),
                (Builtin, "#6c71c4", 0),
                (Operator, "#859900", 0),
                (Punctuation, "#839496", 0),
                (Tag, "#268bd2", 0),
                (Attribute, "#93a1a1", 0),
                (Meta, "#cb4b16", 0),
                (Regex, "#dc322f", 0),
                (Heading, "#cb4b16", B),
                (Link, "#6c71c4", U),
                (Inserted, "#859900", 0),
                (Deleted, "#dc322f", 0),
            ],
        ),
        Theme::build(
            "solarized-light",
            false,
            "#fdf6e3",
            "#657b83",
            &[
                (Comment, "#93a1a1", I),
                (Keyword, "#859900", 0),
                (String, "#2aa198", 0),
                (Escape, "#dc322f", 0),
                (Number, "#d33682", 0),
                (Constant, "#cb4b16", 0),
                (Function, "#268bd2", 0),
                (Type, "#b58900", 0),
                (Variable, "#268bd2", 0),
                (Property, "#268bd2", 0),
                (Builtin, "#6c71c4", 0),
                (Operator, "#859900", 0),
                (Punctuation, "#657b83", 0),
                (Tag, "#268bd2", 0),
                (Attribute, "#93a1a1", 0),
                (Meta, "#cb4b16", 0),
                (Regex, "#dc322f", 0),
                (Heading, "#cb4b16", B),
                (Link, "#6c71c4", U),
                (Inserted, "#859900", 0),
                (Deleted, "#dc322f", 0),
            ],
        ),
    ];
}
