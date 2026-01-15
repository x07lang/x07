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

    if (len < 4) {
        uint32_t result = 0;
        fwrite(&result, sizeof(uint32_t), 1, stdout);
        free(input);
        return 0;
    }

    uint32_t pat_len;
    memcpy(&pat_len, input, 4);

    if (4 + pat_len > len) {
        uint32_t result = 0;
        fwrite(&result, sizeof(uint32_t), 1, stdout);
        free(input);
        return 0;
    }

    char *pattern = malloc(pat_len + 1);
    memcpy(pattern, input + 4, pat_len);
    pattern[pat_len] = '\0';

    size_t text_len = len - 4 - pat_len;
    char *text = malloc(text_len + 1);
    memcpy(text, input + 4 + pat_len, text_len);
    text[text_len] = '\0';

    regex_t regex;
    int ret = regcomp(&regex, pattern, REG_EXTENDED);

    uint32_t count = 0;
    if (ret == 0) {
        regmatch_t match;
        const char *cursor = text;
        while (regexec(&regex, cursor, 1, &match, 0) == 0) {
            count++;
            if (match.rm_eo == 0) {
                cursor++;
                if (*cursor == '\0') break;
            } else {
                cursor += match.rm_eo;
            }
            if (*cursor == '\0') break;
        }
        regfree(&regex);
    }

    fwrite(&count, sizeof(uint32_t), 1, stdout);

    free(pattern);
    free(text);
    free(input);
    return 0;
}
