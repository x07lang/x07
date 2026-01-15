#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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

    uint32_t freq[256] = {0};

    for (size_t i = 0; i < len; i++) {
        freq[input[i]]++;
    }

    uint8_t output[256 * 5];
    size_t out_len = 0;

    for (int j = 0; j < 256; j++) {
        if (freq[j] > 0) {
            output[out_len++] = (uint8_t)j;
            output[out_len++] = (uint8_t)(freq[j] & 0xFF);
            output[out_len++] = (uint8_t)((freq[j] >> 8) & 0xFF);
            output[out_len++] = (uint8_t)((freq[j] >> 16) & 0xFF);
            output[out_len++] = (uint8_t)((freq[j] >> 24) & 0xFF);
        }
    }

    fwrite(output, 1, out_len, stdout);

    free(input);
    return 0;
}
