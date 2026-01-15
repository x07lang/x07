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

    uint32_t cnt = 0;
    int in_word = 0;

    for (size_t i = 0; i < len; i++) {
        uint8_t ch = input[i];
        int is_space = (ch == 32 || ch == 10 || ch == 13 || ch == 9);
        if (is_space) {
            in_word = 0;
        } else if (!in_word) {
            cnt++;
            in_word = 1;
        }
    }

    fwrite(&cnt, sizeof(uint32_t), 1, stdout);

    free(input);
    return 0;
}
