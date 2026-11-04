/*
 * colorfulcode: fast syntax highlighting, C API.
 *
 * Highlighting returns a flat array of tokens that covers the input exactly.
 * Each token has a kind, and cc_theme_style() maps a kind to a color, so a
 * native renderer only has to apply one attribute per token.
 *
 * Conventions:
 * - Languages and themes are referred to by index. Indices are only stable
 *   within one build of the library, so persist ids or names instead.
 * - Source code is passed as UTF-8 bytes plus a length and need not be NUL
 *   terminated. Functions taking source code return NULL for invalid UTF-8.
 * - NULL pointers are accepted everywhere and treated as missing input.
 * - Strings returned as `const char*` are static and must not be freed.
 *   Strings returned as `char*` must be freed with cc_string_free().
 * - All functions are thread safe.
 */

#ifndef COLORFULCODE_H
#define COLORFULCODE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Flags for cc_highlight(): report offsets in UTF-16 code units instead of
 * bytes, as used by NSString, Java, Kotlin and C# strings. */
#define CC_UTF16_OFFSETS (1u << 0)
/* Flags for cc_render_html(): wrap the output in <pre class="cc"><code>. */
#define CC_HTML_WRAP (1u << 1)
/* Flags for cc_render_html(): wrap every line in <span class="cc-line">. */
#define CC_HTML_LINES (1u << 2)
/* Flags for cc_render_ansi(): use the 256 color palette instead of 24 bit colors. */
#define CC_ANSI_256 (1u << 3)

/* Token kinds, the values of cc_token.kind. */
enum {
    CC_KIND_TEXT = 0,
    CC_KIND_COMMENT = 1,
    CC_KIND_KEYWORD = 2,
    CC_KIND_STRING = 3,
    CC_KIND_ESCAPE = 4,
    CC_KIND_NUMBER = 5,
    CC_KIND_CONSTANT = 6,
    CC_KIND_FUNCTION = 7,
    CC_KIND_TYPE = 8,
    CC_KIND_VARIABLE = 9,
    CC_KIND_PROPERTY = 10,
    CC_KIND_BUILTIN = 11,
    CC_KIND_OPERATOR = 12,
    CC_KIND_PUNCTUATION = 13,
    CC_KIND_TAG = 14,
    CC_KIND_ATTRIBUTE = 15,
    CC_KIND_META = 16,
    CC_KIND_REGEX = 17,
    CC_KIND_HEADING = 18,
    CC_KIND_EMPHASIS = 19,
    CC_KIND_STRONG = 20,
    CC_KIND_LINK = 21,
    CC_KIND_INSERTED = 22,
    CC_KIND_DELETED = 23,
    CC_KIND_COUNT = 24
};

/* A highlighted span of the input, [start, end). Offsets are UTF-8 bytes, or
 * UTF-16 code units with CC_UTF16_OFFSETS. */
typedef struct {
    uint32_t start;
    uint32_t end;
    uint8_t kind;
} cc_token;

/* How a token kind is drawn. Flags are 0 or 1. */
typedef struct {
    uint32_t color; /* 0xRRGGBB */
    uint8_t bold;
    uint8_t italic;
    uint8_t underline;
} cc_style;

/* The result of cc_highlight(). Opaque. */
typedef struct cc_tokens cc_tokens;

/* Library version, for example "0.1.0". */
const char* cc_version(void);

/* ---- Languages ---- */

/* Number of languages. Valid indices are 0 to count - 1. */
int32_t cc_language_count(void);

/* Finds a language by id, alias, name or extension, case insensitive
 * ("rust", "ts", "C++", ".py"). Returns -1 if unknown. */
int32_t cc_language_find(const char* name);

/* Guesses the language from a file path such as "src/main.rs".
 * Returns -1 if unknown. */
int32_t cc_language_from_path(const char* path);

/* Guesses the language from a shebang or similar first line.
 * Returns -1 if unknown. */
int32_t cc_language_from_content(const char* code, size_t len);

/* Canonical id of a language, for example "rust". NULL if out of range. */
const char* cc_language_id(int32_t language);

/* Display name of a language, for example "Rust". NULL if out of range. */
const char* cc_language_name(int32_t language);

/* ---- Highlighting ---- */

/* Highlights `len` bytes of UTF-8 `code`. An unknown language index such as
 * -1 means plain text. `flags` may contain CC_UTF16_OFFSETS.
 * Returns NULL for invalid UTF-8. Free the result with cc_tokens_free(). */
cc_tokens* cc_highlight(const char* code, size_t len, int32_t language, uint32_t flags);

/* Number of tokens. 0 for NULL. */
size_t cc_tokens_len(const cc_tokens* tokens);

/* The tokens, valid until cc_tokens_free(). NULL for NULL. */
const cc_token* cc_tokens_data(const cc_tokens* tokens);

/* Frees tokens. NULL is ignored. */
void cc_tokens_free(cc_tokens* tokens);

/* Lowercase name of a token kind, for example "keyword". NULL if out of range. */
const char* cc_kind_name(uint8_t kind);

/* ---- Themes ---- */

/* Number of themes. Valid indices are 0 to count - 1. Index 0 is the default. */
int32_t cc_theme_count(void);

/* Finds a theme by name, case insensitive. Returns -1 if unknown. */
int32_t cc_theme_find(const char* name);

/* Name of a theme, for example "dracula". NULL if out of range. */
const char* cc_theme_name(int32_t theme);

/* 1 for a dark theme, 0 for a light one. Out of range means the default theme. */
uint8_t cc_theme_is_dark(int32_t theme);

/* Background color as 0xRRGGBB. Out of range means the default theme. */
uint32_t cc_theme_background(int32_t theme);

/* Default text color as 0xRRGGBB. Out of range means the default theme. */
uint32_t cc_theme_foreground(int32_t theme);

/* Style of a token kind. Out of range themes mean the default theme and
 * unknown kinds mean CC_KIND_TEXT. */
cc_style cc_theme_style(int32_t theme, uint8_t kind);

/* CSS for the class based HTML, scoped to `selector` (NULL means ".cc").
 * Out of range means the default theme. Free with cc_string_free(). */
char* cc_theme_css(int32_t theme, const char* selector);

/* ---- Rendering ---- */

/* Renders HTML. A valid theme index gives inline styles, -1 gives class names
 * to be styled with cc_theme_css(). `flags` may contain CC_HTML_WRAP and
 * CC_HTML_LINES. Returns NULL for invalid UTF-8 or NUL bytes in the input.
 * Free with cc_string_free(). */
char* cc_render_html(const char* code, size_t len, int32_t language, int32_t theme, uint32_t flags);

/* Renders text with ANSI color escapes for terminals. Out of range themes mean
 * the default theme. `flags` may contain CC_ANSI_256. Returns NULL for invalid
 * UTF-8 or NUL bytes in the input. Free with cc_string_free(). */
char* cc_render_ansi(const char* code, size_t len, int32_t language, int32_t theme, uint32_t flags);

/* Frees a string returned by this library. NULL is ignored. */
void cc_string_free(char* s);

#ifdef __cplusplus
}
#endif

#endif /* COLORFULCODE_H */
