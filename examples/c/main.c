/*
 * Prints a file with syntax highlighting through the colorfulcode C API.
 *
 *     ./highlight <file> [theme]
 */

#include <stdio.h>
#include <stdlib.h>

#include "colorfulcode.h"

/* Reads a whole file into memory. Returns NULL on failure. */
static char* read_file(const char* path, size_t* len) {
    FILE* file = fopen(path, "rb");
    if (!file) {
        return NULL;
    }
    size_t cap = 4096, size = 0;
    char* data = malloc(cap);
    size_t n;
    while (data && (n = fread(data + size, 1, cap - size, file)) > 0) {
        size += n;
        if (size == cap) {
            char* bigger = realloc(data, cap *= 2);
            if (!bigger) {
                free(data);
            }
            data = bigger;
        }
    }
    if (data && ferror(file)) {
        free(data);
        data = NULL;
    }
    fclose(file);
    *len = size;
    return data;
}

int main(int argc, char** argv) {
    if (argc < 2 || argc > 3) {
        fprintf(stderr, "usage: %s <file> [theme]\n", argv[0]);
        return 2;
    }
    int32_t theme = argc == 3 ? cc_theme_find(argv[2]) : 0;
    if (theme < 0) {
        fprintf(stderr, "unknown theme: %s\n", argv[2]);
        return 2;
    }
    size_t len;
    char* code = read_file(argv[1], &len);
    if (!code) {
        perror(argv[1]);
        return 1;
    }

    int32_t language = cc_language_from_path(argv[1]);
    if (language < 0) {
        language = cc_language_from_content(code, len);
    }
    cc_tokens* tokens = cc_highlight(code, len, language, 0);
    if (!tokens) {
        fprintf(stderr, "%s: not valid UTF-8\n", argv[1]);
        free(code);
        return 1;
    }

    cc_style styles[CC_KIND_COUNT];
    for (int kind = 0; kind < CC_KIND_COUNT; kind++) {
        styles[kind] = cc_theme_style(theme, (uint8_t)kind);
    }

    const cc_token* data = cc_tokens_data(tokens);
    for (size_t i = 0; i < cc_tokens_len(tokens); i++) {
        cc_token t = data[i];
        const char* text = code + t.start;
        size_t n = t.end - t.start;
        /* Plain text keeps the terminal's own color. */
        if (t.kind == CC_KIND_TEXT) {
            fwrite(text, 1, n, stdout);
            continue;
        }
        cc_style s = styles[t.kind];
        printf("\x1b[%s%s%s38;2;%u;%u;%um", s.bold ? "1;" : "", s.italic ? "3;" : "", s.underline ? "4;" : "",
               (unsigned)(s.color >> 16) & 0xff, (unsigned)(s.color >> 8) & 0xff, (unsigned)s.color & 0xff);
        fwrite(text, 1, n, stdout);
        fputs("\x1b[0m", stdout);
    }

    cc_tokens_free(tokens);
    free(code);
    return 0;
}
