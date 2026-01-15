#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

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

    if (len == 0) {
        free(input);
        return 0;
    }

    uint8_t *output = malloc(len * 2);
    size_t out_len = 0;

    uint8_t cur = input[0];
    uint8_t cnt = 1;

    for (size_t i = 1; i < len; i++) {
        uint8_t x = input[i];
        if (x == cur) {
            if (cnt < 255) {
                cnt++;
            } else {
                output[out_len++] = cnt;
                output[out_len++] = cur;
                cnt = 1;
            }
        } else {
            output[out_len++] = cnt;
            output[out_len++] = cur;
            cur = x;
            cnt = 1;
        }
    }

    output[out_len++] = cnt;
    output[out_len++] = cur;

    fwrite(output, 1, out_len, stdout);

    free(input);
    free(output);
    return 0;
}
