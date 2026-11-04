//! C ABI for colorfulcode, for native apps written in Swift, Kotlin, C++, C#
//! and friends. The matching header is `include/colorfulcode.h`.
//!
//! Highlighting returns a flat array of `cc_token`s that covers the input,
//! and `cc_theme_style` maps each token kind to a color, so drawing comes
//! down to one attribute per token.
//!
//! Conventions:
//! - Languages and themes are referred to by index. Indices are only stable
//!   within one build of the library, so persist ids or names instead.
//! - Input strings must be UTF-8. Functions taking source code return NULL
//!   for invalid UTF-8 rather than guessing.
//! - Null pointers are accepted everywhere and treated as missing input.
//! - Panics never cross the boundary: they are caught, or abort in release
//!   builds where `panic = "abort"`.

use colorfulcode::{ansi, highlight, html, Kind, Language, Theme};
use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
use std::sync::OnceLock;

/// `cc_highlight` flag: report offsets in UTF-16 code units instead of bytes.
pub const CC_UTF16_OFFSETS: u32 = 1 << 0;
/// `cc_render_html` flag: wrap the output in `<pre class="cc"><code>`.
pub const CC_HTML_WRAP: u32 = 1 << 1;
/// `cc_render_html` flag: wrap every line in `<span class="cc-line">`.
pub const CC_HTML_LINES: u32 = 1 << 2;
/// `cc_render_ansi` flag: use the 256 color palette instead of 24 bit colors.
pub const CC_ANSI_256: u32 = 1 << 3;

/// A highlighted span of the input, `start..end`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CcToken {
    pub start: u32,
    pub end: u32,
    /// A `Kind` as its numeric value.
    pub kind: u8,
}

/// How a token kind is drawn.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CcStyle {
    /// `0xRRGGBB`.
    pub color: u32,
    pub bold: u8,
    pub italic: u8,
    pub underline: u8,
}

/// The result of `cc_highlight`. Opaque to C.
pub struct CcTokens(Vec<CcToken>);

/// NUL terminated copies of static strings, built on first use and kept for
/// the life of the program so C callers never free them.
struct CStrings(OnceLock<Vec<CString>>);

impl CStrings {
    const fn new() -> CStrings {
        CStrings(OnceLock::new())
    }

    fn get(&self, index: Option<usize>, init: impl FnOnce() -> Vec<&'static str>) -> *const c_char {
        let all = self.0.get_or_init(|| init().into_iter().map(|s| CString::new(s).unwrap_or_default()).collect());
        index.and_then(|i| all.get(i)).map_or(ptr::null(), |s| s.as_ptr())
    }
}

/// Runs `f`, returning `fallback` if it panics so nothing unwinds into C.
fn guard<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(fallback)
}

fn index(i: i32) -> Option<usize> {
    usize::try_from(i).ok()
}

fn to_index(i: Option<usize>) -> i32 {
    i.and_then(|i| i32::try_from(i).ok()).unwrap_or(-1)
}

fn language_at(i: i32) -> Language {
    index(i).and_then(|i| Language::all().get(i)).copied().unwrap_or_else(Language::plain)
}

fn language_index(language: Option<Language>) -> i32 {
    to_index(language.and_then(|l| Language::all().iter().position(|x| *x == l)))
}

fn theme_at(i: i32) -> Option<&'static Theme> {
    index(i).and_then(|i| Theme::all().get(i))
}

fn theme_or_default(i: i32) -> &'static Theme {
    theme_at(i).unwrap_or(&Theme::all()[0])
}

/// Borrows a NUL terminated UTF-8 string.
unsafe fn str_arg<'a>(s: *const c_char) -> Option<&'a str> {
    if s.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(s) }.to_str().ok()
}

/// Borrows `len` bytes of source code. `None` for invalid UTF-8, input of
/// 4 GiB or more, or a null pointer with a nonzero length.
unsafe fn source<'a>(code: *const c_char, len: usize) -> Option<&'a str> {
    if code.is_null() {
        return (len == 0).then_some("");
    }
    if len > u32::MAX as usize {
        return None;
    }
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(code.cast::<u8>(), len) }).ok()
}

fn into_c_string(s: String) -> *mut c_char {
    CString::new(s).map_or(ptr::null_mut(), CString::into_raw)
}

/// Library version, for example `"0.1.0"`.
#[no_mangle]
pub extern "C" fn cc_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// Number of languages.
#[no_mangle]
pub extern "C" fn cc_language_count() -> i32 {
    to_index(Some(Language::all().len()))
}

/// Finds a language by id, alias, name or extension. Returns -1 if unknown.
///
/// # Safety
///
/// `name` must be null or a NUL terminated string.
#[no_mangle]
pub unsafe extern "C" fn cc_language_find(name: *const c_char) -> i32 {
    language_index(unsafe { str_arg(name) }.and_then(Language::find))
}

/// Guesses the language from a file path. Returns -1 if unknown.
///
/// # Safety
///
/// `path` must be null or a NUL terminated string.
#[no_mangle]
pub unsafe extern "C" fn cc_language_from_path(path: *const c_char) -> i32 {
    language_index(unsafe { str_arg(path) }.and_then(Language::from_path))
}

/// Guesses the language from a shebang or similar first line. Returns -1 if unknown.
///
/// # Safety
///
/// `code` must be null or point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn cc_language_from_content(code: *const c_char, len: usize) -> i32 {
    language_index(unsafe { source(code, len) }.and_then(Language::from_content))
}

/// Canonical id of a language, for example `"rust"`. NULL if out of range.
#[no_mangle]
pub extern "C" fn cc_language_id(language: i32) -> *const c_char {
    static IDS: CStrings = CStrings::new();
    IDS.get(index(language), || Language::all().iter().map(|l| l.id()).collect())
}

/// Display name of a language, for example `"Rust"`. NULL if out of range.
#[no_mangle]
pub extern "C" fn cc_language_name(language: i32) -> *const c_char {
    static NAMES: CStrings = CStrings::new();
    NAMES.get(index(language), || Language::all().iter().map(|l| l.name()).collect())
}

/// Highlights `len` bytes of UTF-8 `code`. An unknown language index means
/// plain text. Returns NULL for invalid UTF-8. Free with `cc_tokens_free`.
///
/// # Safety
///
/// `code` must be null or point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn cc_highlight(code: *const c_char, len: usize, language: i32, flags: u32) -> *mut CcTokens {
    let Some(code) = (unsafe { source(code, len) }) else {
        return ptr::null_mut();
    };
    guard(ptr::null_mut(), || {
        let highlighted = highlight(code, language_at(language));
        let tokens = if flags & CC_UTF16_OFFSETS != 0 { highlighted.utf16_tokens() } else { highlighted.into_tokens() };
        let tokens = tokens.iter().map(|t| CcToken { start: t.start, end: t.end, kind: t.kind as u8 }).collect();
        Box::into_raw(Box::new(CcTokens(tokens)))
    })
}

/// Number of tokens. 0 for NULL.
///
/// # Safety
///
/// `tokens` must be null or come from `cc_highlight` and not be freed.
#[no_mangle]
pub unsafe extern "C" fn cc_tokens_len(tokens: *const CcTokens) -> usize {
    unsafe { tokens.as_ref() }.map_or(0, |t| t.0.len())
}

/// The tokens, valid until `cc_tokens_free`. NULL for NULL.
///
/// # Safety
///
/// `tokens` must be null or come from `cc_highlight` and not be freed.
#[no_mangle]
pub unsafe extern "C" fn cc_tokens_data(tokens: *const CcTokens) -> *const CcToken {
    unsafe { tokens.as_ref() }.map_or(ptr::null(), |t| t.0.as_ptr())
}

/// Frees tokens. NULL is ignored.
///
/// # Safety
///
/// `tokens` must be null or come from `cc_highlight` and not be freed yet.
#[no_mangle]
pub unsafe extern "C" fn cc_tokens_free(tokens: *mut CcTokens) {
    if !tokens.is_null() {
        drop(unsafe { Box::from_raw(tokens) });
    }
}

/// Lowercase name of a token kind, for example `"keyword"`. NULL if out of range.
#[no_mangle]
pub extern "C" fn cc_kind_name(kind: u8) -> *const c_char {
    static NAMES: CStrings = CStrings::new();
    NAMES.get(Some(kind.into()), || Kind::ALL.iter().map(|k| k.name()).collect())
}

/// Number of themes.
#[no_mangle]
pub extern "C" fn cc_theme_count() -> i32 {
    to_index(Some(Theme::all().len()))
}

/// Finds a theme by name (case insensitive). Returns -1 if unknown.
///
/// # Safety
///
/// `name` must be null or a NUL terminated string.
#[no_mangle]
pub unsafe extern "C" fn cc_theme_find(name: *const c_char) -> i32 {
    let theme = unsafe { str_arg(name) }.and_then(Theme::find);
    to_index(theme.and_then(|t| Theme::all().iter().position(|x| ptr::eq(x, t))))
}

/// Name of a theme, for example `"dracula"`. NULL if out of range.
#[no_mangle]
pub extern "C" fn cc_theme_name(theme: i32) -> *const c_char {
    static NAMES: CStrings = CStrings::new();
    NAMES.get(index(theme), || Theme::all().iter().map(|t| t.name).collect())
}

/// 1 for a dark theme, 0 for a light one. Out of range means the default theme.
#[no_mangle]
pub extern "C" fn cc_theme_is_dark(theme: i32) -> u8 {
    theme_or_default(theme).dark.into()
}

/// Background color as `0xRRGGBB`. Out of range means the default theme.
#[no_mangle]
pub extern "C" fn cc_theme_background(theme: i32) -> u32 {
    theme_or_default(theme).background.to_u32()
}

/// Default text color as `0xRRGGBB`. Out of range means the default theme.
#[no_mangle]
pub extern "C" fn cc_theme_foreground(theme: i32) -> u32 {
    theme_or_default(theme).foreground.to_u32()
}

/// Style of a token kind. Out of range themes mean the default theme and
/// unknown kinds mean plain text.
#[no_mangle]
pub extern "C" fn cc_theme_style(theme: i32, kind: u8) -> CcStyle {
    let style = theme_or_default(theme).style(Kind::from_u8(kind).unwrap_or(Kind::Text));
    CcStyle {
        color: style.color.to_u32(),
        bold: style.bold.into(),
        italic: style.italic.into(),
        underline: style.underline.into(),
    }
}

/// CSS for the class based HTML, scoped to `selector` (NULL means `".cc"`).
/// Free with `cc_string_free`.
///
/// # Safety
///
/// `selector` must be null or a NUL terminated string.
#[no_mangle]
pub unsafe extern "C" fn cc_theme_css(theme: i32, selector: *const c_char) -> *mut c_char {
    let selector = if selector.is_null() { Some(".cc") } else { unsafe { str_arg(selector) } };
    let Some(selector) = selector else {
        return ptr::null_mut();
    };
    guard(ptr::null_mut(), || into_c_string(theme_or_default(theme).to_css(selector)))
}

/// Renders HTML. A valid theme index gives inline styles, -1 gives class
/// names. Takes `CC_HTML_*` flags. Returns NULL for invalid UTF-8 or NUL
/// bytes in the input. Free with `cc_string_free`.
///
/// # Safety
///
/// `code` must be null or point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn cc_render_html(
    code: *const c_char,
    len: usize,
    language: i32,
    theme: i32,
    flags: u32,
) -> *mut c_char {
    let Some(code) = (unsafe { source(code, len) }) else {
        return ptr::null_mut();
    };
    guard(ptr::null_mut(), || {
        let options = html::Options {
            theme: theme_at(theme),
            wrap: flags & CC_HTML_WRAP != 0,
            lines: flags & CC_HTML_LINES != 0,
            ..Default::default()
        };
        into_c_string(highlight(code, language_at(language)).to_html(&options))
    })
}

/// Renders text with ANSI color escapes. Out of range themes mean the
/// default theme. Takes `CC_ANSI_256`. Returns NULL for invalid UTF-8 or NUL
/// bytes in the input. Free with `cc_string_free`.
///
/// # Safety
///
/// `code` must be null or point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn cc_render_ansi(
    code: *const c_char,
    len: usize,
    language: i32,
    theme: i32,
    flags: u32,
) -> *mut c_char {
    let Some(code) = (unsafe { source(code, len) }) else {
        return ptr::null_mut();
    };
    guard(ptr::null_mut(), || {
        let mode = if flags & CC_ANSI_256 != 0 { ansi::ColorMode::Ansi256 } else { ansi::ColorMode::TrueColor };
        into_c_string(highlight(code, language_at(language)).to_ansi(theme_or_default(theme), mode))
    })
}

/// Frees a string returned by this library. NULL is ignored.
///
/// # Safety
///
/// `s` must be null or come from this library and not be freed yet.
#[no_mangle]
pub unsafe extern "C" fn cc_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}
