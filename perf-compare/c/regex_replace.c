#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <regex.h>

int main(void) {
    uint8_t *input = NULL;
    size_t capacity = 0;
    size_t len = 0;

    int c;
    while ((c = getchar()) != EOF) {
        if (len >= capacity) {
            capacity = capacity ? capacity * 2 : 4096;
            input = realloc(input, capacity);
        }
        input[len++] = (uint8_t)c;
    }

    if (len < 8) {
        fwrite(input, 1, len, stdout);
        free(input);
        return 0;
    }

    uint32_t pat_len, repl_len;
    memcpy(&pat_len, input, 4);
    memcpy(&repl_len, input + 4, 4);

    if (8 + pat_len + repl_len > len) {
        size_t text_start = 8 + pat_len + repl_len;
        if (text_start <= len) {
            fwrite(input + text_start, 1, len - text_start, stdout);
        }
        free(input);
        return 0;
    }

    char *pattern = malloc(pat_len + 1);
    memcpy(pattern, input + 8, pat_len);
    pattern[pat_len] = '\0';

    char *replacement = malloc(repl_len + 1);
    memcpy(replacement, input + 8 + pat_len, repl_len);
    replacement[repl_len] = '\0';

    size_t text_len = len - 8 - pat_len - repl_len;
    char *text = malloc(text_len + 1);
    memcpy(text, input + 8 + pat_len + repl_len, text_len);
    text[text_len] = '\0';

    regex_t regex;
    int ret = regcomp(&regex, pattern, REG_EXTENDED);

    if (ret != 0) {
        fwrite(text, 1, text_len, stdout);
        free(pattern);
        free(replacement);
        free(text);
        free(input);
        return 0;
    }

    size_t out_capacity = text_len * 2 + 1024;
    char *output = malloc(out_capacity);
    size_t out_len = 0;

    regmatch_t match;
    const char *cursor = text;
    size_t remaining = text_len;

    while (regexec(&regex, cursor, 1, &match, 0) == 0 && remaining > 0) {
        size_t prefix_len = match.rm_so;
        size_t match_len = match.rm_eo - match.rm_so;

        size_t needed = out_len + prefix_len + repl_len + remaining;
        if (needed > out_capacity) {
            out_capacity = needed * 2;
            output = realloc(output, out_capacity);
        }

        memcpy(output + out_len, cursor, prefix_len);
        out_len += prefix_len;

        memcpy(output + out_len, replacement, repl_len);
        out_len += repl_len;

        if (match_len == 0) {
            if (remaining > 0) {
                output[out_len++] = *cursor;
                cursor++;
                remaining--;
            }
            if (remaining == 0) break;
        } else {
            cursor += match.rm_eo;
            remaining -= match.rm_eo;
        }
    }

    if (remaining > 0) {
        size_t needed = out_len + remaining;
        if (needed > out_capacity) {
            out_capacity = needed;
            output = realloc(output, out_capacity);
        }
        memcpy(output + out_len, cursor, remaining);
        out_len += remaining;
    }

    regfree(&regex);

    fwrite(output, 1, out_len, stdout);

    free(output);
    free(pattern);
    free(replacement);
    free(text);
    free(input);
    return 0;
}
