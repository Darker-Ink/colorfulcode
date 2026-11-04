//! A tiny C style ABI over colorfulcode for the JavaScript package.
//!
//! There is no bindings generator involved. JavaScript writes UTF-8 input into
//! a buffer it gets from [`cc_input`], calls an exported function, and reads
//! the result from [`cc_output_ptr`]. Buffers are reused between calls so a
//! highlight does not allocate once warmed up.

use colorfulcode::{html, utf16, Color, Kind, Language, Style, Theme, Token};
use std::cell::RefCell;
use std::fmt::Write;

#[derive(Default)]
struct State {
    input: Vec<u8>,
    lines: Vec<u32>,
    tokens: Vec<Token>,
    output: Vec<u8>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    /// Themes defined at runtime, indexed after the built in ones.
    static CUSTOM_THEMES: RefCell<Vec<&'static Theme>> = const { RefCell::new(Vec::new()) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Flag for [`cc_highlight`]: wrap the output in `<pre class="cc"><code>`.
pub const WRAP: u32 = 1;
/// Flag for [`cc_highlight`]: wrap every line in `<span class="cc-line">`.
pub const LINES: u32 = 2;
/// Flag for [`cc_highlight`]: add line numbers.
pub const LINE_NUMBERS: u32 = 4;

/// Makes sure the input buffer holds `capacity` bytes and returns it.
#[no_mangle]
pub extern "C" fn cc_input(capacity: usize) -> *mut u8 {
    with(|s| {
        // Only grow: zeroing the buffer on every call would cost more than the highlighting.
        if s.input.len() < capacity {
            s.input.resize(capacity, 0);
        }
        s.input.as_mut_ptr()
    })
}

/// Makes sure the line buffer holds `count` numbers and returns it.
#[no_mangle]
pub extern "C" fn cc_lines(count: usize) -> *mut u32 {
    with(|s| {
        s.lines.clear();
        s.lines.resize(count, 0);
        s.lines.as_mut_ptr()
    })
}

#[no_mangle]
pub extern "C" fn cc_output_ptr() -> *const u8 {
    with(|s| s.output.as_ptr())
}

fn input_str(input: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(input)
}

fn language(index: i32) -> Language {
    usize::try_from(index).ok().and_then(|i| Language::all().get(i).copied()).unwrap_or_else(Language::plain)
}

fn theme(index: i32) -> Option<&'static Theme> {
    let i = usize::try_from(index).ok()?;
    let builtin = Theme::all();
    match builtin.get(i) {
        Some(t) => Some(t),
        None => CUSTOM_THEMES.with(|c| c.borrow().get(i - builtin.len()).copied()),
    }
}

fn all_themes() -> Vec<&'static Theme> {
    let mut themes: Vec<&'static Theme> = Theme::all().iter().collect();
    CUSTOM_THEMES.with(|c| themes.extend(c.borrow().iter().copied()));
    themes
}

/// Defines a theme from the input, a small line based format written by the
/// JavaScript side:
///
/// ```text
/// name
/// dark|light
/// #background #foreground
/// keyword #ff79c6 b
/// comment #6272a4 i
/// ```
///
/// Flags are any of `b`, `i` and `u`. Returns the theme index, or -1 if the
/// input is malformed. Defining a name again replaces the earlier theme.
#[no_mangle]
pub extern "C" fn cc_theme_define(len: usize) -> i32 {
    let text = with(|s| String::from_utf8_lossy(&s.input[..len]).into_owned());
    let Some(theme) = parse_theme(&text) else {
        return -1;
    };
    let theme: &'static Theme = Box::leak(Box::new(theme));
    let builtin = Theme::all().len();
    CUSTOM_THEMES.with(|c| {
        let mut custom = c.borrow_mut();
        let index = match custom.iter().position(|t| t.name == theme.name) {
            Some(i) => {
                custom[i] = theme;
                i
            }
            None => {
                custom.push(theme);
                custom.len() - 1
            }
        };
        (builtin + index) as i32
    })
}

fn parse_theme(text: &str) -> Option<Theme> {
    let mut lines = text.lines();
    let name: &'static str = Box::leak(lines.next()?.trim().to_string().into_boxed_str());
    let dark = lines.next()?.trim() == "dark";
    let mut colors = lines.next()?.split_whitespace();
    let background = parse_color(colors.next()?)?;
    let foreground = parse_color(colors.next()?)?;
    let mut styles = Vec::new();
    for line in lines {
        let mut parts = line.split_whitespace();
        let (Some(kind), Some(color)) = (parts.next(), parts.next()) else {
            continue;
        };
        let (Some(kind), Some(color)) = (Kind::from_name(kind), parse_color(color)) else {
            continue;
        };
        let flags = parts.next().unwrap_or("");
        let mut style = Style::new(color);
        style.bold = flags.contains('b');
        style.italic = flags.contains('i');
        style.underline = flags.contains('u');
        styles.push((kind, style));
    }
    Some(Theme::new(name, dark, background, foreground, &styles))
}

fn parse_color(s: &str) -> Option<Color> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    (hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then(|| Color::hex(hex))
}

fn index_of(lang: Option<Language>) -> i32 {
    lang.and_then(|l| Language::all().iter().position(|x| *x == l)).map_or(-1, |i| i as i32)
}

/// Looks up the language named by the first `len` input bytes. Returns its index or -1.
#[no_mangle]
pub extern "C" fn cc_language_find(len: usize) -> i32 {
    with(|s| index_of(Language::find(&input_str(&s.input[..len]))))
}

/// Detects a language from a file path in the input. Returns its index or -1.
#[no_mangle]
pub extern "C" fn cc_language_from_path(len: usize) -> i32 {
    with(|s| index_of(Language::from_path(&input_str(&s.input[..len]))))
}

/// Detects a language from file contents in the input. Returns its index or -1.
#[no_mangle]
pub extern "C" fn cc_language_from_content(len: usize) -> i32 {
    with(|s| index_of(Language::from_content(&input_str(&s.input[..len]))))
}

/// Guesses the language of the code in the input from its content alone, see
/// `Language::detect`. Returns its index or -1 when unsure.
#[no_mangle]
pub extern "C" fn cc_language_detect(len: usize) -> i32 {
    with(|s| index_of(Language::detect(&input_str(&s.input[..len]))))
}

/// Highlights the input as HTML. `theme` is a theme index for inline styles
/// or -1 for class names. The first `line_count` numbers of the line buffer
/// are highlighted lines. Returns the output length.
#[no_mangle]
pub extern "C" fn cc_highlight(len: usize, lang: i32, theme_index: i32, flags: u32, line_count: usize) -> usize {
    with(|s| {
        let code = input_str(&s.input[..len]);
        colorfulcode::highlight_into(&code, language(lang), &mut s.tokens);
        let options = html::Options {
            theme: theme(theme_index),
            wrap: flags & WRAP != 0,
            lines: flags & LINES != 0,
            line_numbers: flags & LINE_NUMBERS != 0,
            highlight_lines: &s.lines[..line_count.min(s.lines.len())],
        };
        let out = html::render(&code, &s.tokens, &options);
        s.output = out.into_bytes();
        s.output.len()
    })
}

/// Tokenizes the input. The output holds `(start, end, kind)` triples of
/// little endian `u32`s with offsets in UTF-16 code units, as JavaScript
/// strings use. Returns the number of tokens.
#[no_mangle]
pub extern "C" fn cc_tokenize(len: usize, lang: i32) -> usize {
    with(|s| {
        let code = input_str(&s.input[..len]);
        colorfulcode::highlight_into(&code, language(lang), &mut s.tokens);
        s.output.clear();
        s.output.reserve(s.tokens.len() * 12);
        let out = &mut s.output;
        utf16::convert_with(&code, &s.tokens, |t| {
            out.extend_from_slice(&t.start.to_le_bytes());
            out.extend_from_slice(&t.end.to_le_bytes());
            out.extend_from_slice(&(t.kind as u32).to_le_bytes());
        });
        s.tokens.len()
    })
}

/// CSS for the class based output of a theme, scoped to the selector in the input.
#[no_mangle]
pub extern "C" fn cc_theme_css(theme_index: i32, len: usize) -> usize {
    with(|s| {
        let selector = input_str(&s.input[..len]).into_owned();
        let css = theme(theme_index).map(|t| t.to_css(&selector)).unwrap_or_default();
        s.output = css.into_bytes();
        s.output.len()
    })
}

/// Writes JSON describing every language, theme and token kind.
#[no_mangle]
pub extern "C" fn cc_metadata() -> usize {
    with(|s| {
        s.output = metadata().into_bytes();
        s.output.len()
    })
}

fn metadata() -> String {
    let mut json = String::from("{\"languages\":[");
    for (i, lang) in Language::all().iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        let _ = write!(json, "{{\"id\":\"{}\",\"name\":", lang.id());
        push_json_str(&mut json, lang.name());
        json.push_str(",\"aliases\":");
        push_json_list(&mut json, lang.aliases());
        json.push_str(",\"extensions\":");
        push_json_list(&mut json, lang.extensions());
        json.push_str(",\"filenames\":");
        push_json_list(&mut json, lang.filenames());
        json.push('}');
    }
    json.push_str("],\"themes\":[");
    for (i, theme) in all_themes().into_iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        let _ = write!(
            json,
            "{{\"name\":\"{}\",\"dark\":{},\"background\":\"{}\",\"foreground\":\"{}\",\"styles\":{{",
            theme.name, theme.dark, theme.background, theme.foreground
        );
        for (k, kind) in colorfulcode::Kind::ALL.iter().enumerate() {
            let st = theme.style(*kind);
            if k > 0 {
                json.push(',');
            }
            let _ = write!(
                json,
                "\"{}\":{{\"color\":\"{}\",\"bold\":{},\"italic\":{},\"underline\":{}}}",
                kind.name(),
                st.color,
                st.bold,
                st.italic,
                st.underline
            );
        }
        json.push_str("}}");
    }
    json.push_str("],\"kinds\":");
    let kinds: Vec<&str> = colorfulcode::Kind::ALL.iter().map(|k| k.name()).collect();
    push_json_list(&mut json, &kinds);
    json.push('}');
    json
}

fn push_json_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn push_json_list(out: &mut String, items: &[&str]) {
    out.push('[');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_json_str(out, item);
    }
    out.push(']');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_input(s: &str) -> usize {
        let ptr = cc_input(s.len());
        unsafe { std::ptr::copy_nonoverlapping(s.as_ptr(), ptr, s.len()) };
        s.len()
    }

    fn output(len: usize) -> Vec<u8> {
        unsafe { std::slice::from_raw_parts(cc_output_ptr(), len).to_vec() }
    }

    #[test]
    fn highlight_roundtrip() {
        let len = write_input("rust");
        let lang = cc_language_find(len);
        assert!(lang >= 0);
        let len = write_input("fn main() {}");
        let out = cc_highlight(len, lang, -1, 0, 0);
        let html = String::from_utf8(output(out)).unwrap();
        assert!(html.contains("<span class=\"cc-keyword\">fn</span>"));
    }

    #[test]
    fn detection() {
        let len = write_input("def greet(name):\n    print(f\"hi {name}\")\n");
        let index = cc_language_detect(len);
        assert_eq!(Language::all()[index as usize].id(), "python");
        let len = write_input("just some words");
        assert_eq!(cc_language_detect(len), -1);
    }

    #[test]
    fn tokens_use_utf16_offsets() {
        let len = write_input("javascript");
        let lang = cc_language_find(len);
        let len = write_input("'é🎨' + x");
        let count = cc_tokenize(len, lang);
        let raw = output(count * 12);
        let words: Vec<u32> = raw.chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
        // The string token spans 5 UTF-16 code units: ' é (1) 🎨 (2) '
        assert_eq!(&words[..3], &[0, 5, colorfulcode::Kind::String as u32]);
    }

    #[test]
    fn custom_themes() {
        let len = write_input("discord\ndark\n#313338 #dbdee1\nkeyword #ff7ab2 b\nbogus line\n");
        let index = cc_theme_define(len);
        assert_eq!(index as usize, Theme::all().len());
        let t = theme(index).unwrap();
        assert_eq!(t.style(Kind::Keyword).color, Color::hex("#ff7ab2"));
        assert!(t.style(Kind::Keyword).bold);
        // Redefining replaces in place.
        let len = write_input("discord\nlight\n#ffffff #000000\n");
        assert_eq!(cc_theme_define(len), index);
        assert!(!theme(index).unwrap().dark);
        let len = write_input("broken");
        assert_eq!(cc_theme_define(len), -1);
    }

    #[test]
    fn metadata_is_json_like() {
        let len = cc_metadata();
        let json = String::from_utf8(output(len)).unwrap();
        assert!(json.starts_with("{\"languages\":[") && json.ends_with('}'));
        assert!(json.contains("\"id\":\"typescript\""));
    }
}
