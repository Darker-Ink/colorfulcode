use colorfulcode_ffi::*;
use std::ffi::{c_char, CStr};
use std::ptr;

fn c(s: &CStr) -> *const c_char {
    s.as_ptr()
}

fn string(p: *const c_char) -> &'static str {
    assert!(!p.is_null());
    unsafe { CStr::from_ptr(p) }.to_str().unwrap()
}

fn tokens(code: &str, language: i32, flags: u32) -> Vec<CcToken> {
    unsafe {
        let handle = cc_highlight(code.as_ptr().cast(), code.len(), language, flags);
        assert!(!handle.is_null());
        let out = std::slice::from_raw_parts(cc_tokens_data(handle), cc_tokens_len(handle)).to_vec();
        cc_tokens_free(handle);
        out
    }
}

#[test]
fn layout() {
    assert_eq!(std::mem::size_of::<CcToken>(), 12);
    assert_eq!(std::mem::size_of::<CcStyle>(), 8);
}

#[test]
fn languages() {
    unsafe {
        let rust = cc_language_find(c(c"rust"));
        assert!(rust >= 0 && rust < cc_language_count());
        assert_eq!(string(cc_language_id(rust)), "rust");
        assert_eq!(string(cc_language_name(rust)), "Rust");
        assert_eq!(cc_language_from_path(c(c"src/main.rs")), rust);
        assert_eq!(cc_language_find(c(c"no such language")), -1);
        assert_eq!(cc_language_find(ptr::null()), -1);
        assert!(cc_language_id(-1).is_null());
        assert!(cc_language_name(cc_language_count()).is_null());
        let script = "#!/bin/sh\necho hi\n";
        assert_eq!(string(cc_language_id(cc_language_from_content(script.as_ptr().cast(), script.len()))), "bash");
        let code = "package main\n\nfunc main() {\n\tfmt.Println(\"hi\")\n}\n";
        assert_eq!(string(cc_language_id(cc_language_detect(code.as_ptr().cast(), code.len()))), "go");
        assert_eq!(cc_language_detect(c"hello".as_ptr(), 5), -1);
        assert_eq!(cc_language_detect(ptr::null(), 0), -1);
        assert_eq!(cc_language_detect(c"\xff\xfe".as_ptr(), 2), -1);
    }
}

#[test]
fn highlighting() {
    let rust = unsafe { cc_language_find(c(c"rust")) };
    let code = "let s = \"é😀\"; // ok";
    let bytes = tokens(code, rust, 0);
    assert_eq!(bytes.first().unwrap().start, 0);
    assert_eq!(bytes.last().unwrap().end as usize, code.len());
    assert_eq!(string(cc_kind_name(bytes[0].kind)), "keyword");

    let utf16 = tokens(code, rust, CC_UTF16_OFFSETS);
    assert_eq!(utf16.len(), bytes.len());
    assert_eq!(utf16.last().unwrap().end as usize, code.encode_utf16().count());

    // Unknown languages fall back to plain text.
    assert_eq!(tokens("x", -1, 0), [CcToken { start: 0, end: 1, kind: 0 }]);
    assert!(tokens("", rust, 0).is_empty());
}

#[test]
fn bad_input() {
    unsafe {
        let invalid = b"fn \xff";
        assert!(cc_highlight(invalid.as_ptr().cast(), invalid.len(), 0, 0).is_null());
        assert!(cc_highlight(ptr::null(), 3, 0, 0).is_null());
        let empty = cc_highlight(ptr::null(), 0, 0, 0);
        assert_eq!(cc_tokens_len(empty), 0);
        cc_tokens_free(empty);
        assert_eq!(cc_tokens_len(ptr::null()), 0);
        assert!(cc_tokens_data(ptr::null()).is_null());
        cc_tokens_free(ptr::null_mut());
        cc_string_free(ptr::null_mut());
        assert!(cc_render_html(invalid.as_ptr().cast(), invalid.len(), 0, -1, 0).is_null());
    }
}

#[test]
fn themes() {
    unsafe {
        let count = cc_theme_count();
        assert!(count > 0);
        assert_eq!(string(cc_theme_name(0)), "colorful");
        let dracula = cc_theme_find(c(c"Dracula"));
        assert!(dracula > 0 && dracula < count);
        assert_eq!(cc_theme_background(dracula), 0x282a36);
        assert_eq!(cc_theme_foreground(dracula), 0xf8f8f2);
        assert_eq!(cc_theme_is_dark(dracula), 1);
        assert_eq!(cc_theme_find(c(c"nope")), -1);
        assert!(cc_theme_name(count).is_null());

        let text = cc_theme_style(dracula, 0);
        assert_eq!(text.color, 0xf8f8f2);
        assert_ne!(cc_theme_style(dracula, 2).color, text.color);
        assert_eq!(cc_theme_style(dracula, 255), text);
        assert_eq!(cc_theme_style(-1, 0), cc_theme_style(0, 0));
        assert!(cc_kind_name(255).is_null());
    }
}

#[test]
fn rendering() {
    unsafe {
        let js = cc_language_find(c(c"js"));
        let code = "let a = '<b>';";
        let html = cc_render_html(code.as_ptr().cast(), code.len(), js, -1, CC_HTML_WRAP);
        let text = string(html);
        assert!(text.starts_with("<pre class=\"cc\"><code>"));
        assert!(text.contains("<span class=\"cc-keyword\">let</span>"));
        assert!(text.contains("&lt;b&gt;"));
        cc_string_free(html);

        let inline = cc_render_html(code.as_ptr().cast(), code.len(), js, 0, 0);
        assert!(string(inline).contains("style=\"color:#"));
        cc_string_free(inline);

        let ansi = cc_render_ansi(code.as_ptr().cast(), code.len(), js, 0, CC_ANSI_256);
        assert!(string(ansi).contains("\x1b[38;5;"));
        cc_string_free(ansi);

        let css = cc_theme_css(0, ptr::null());
        assert!(string(css).contains(".cc .cc-keyword{"));
        cc_string_free(css);

        assert_eq!(string(cc_version()), env!("CARGO_PKG_VERSION"));
    }
}
