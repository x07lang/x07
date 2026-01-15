#include <stdint.h>
#include <stdio.h>

int main(void) {
    uint32_t n;
    if (fread(&n, sizeof(uint32_t), 1, stdin) != 1) {
        return 1;
    }

    uint32_t result;
    if (n < 2) {
        result = n;
    } else {
        uint32_t a = 0;
        uint32_t b = 1;
        for (uint32_t i = 2; i <= n; i++) {
            uint32_t tmp = a + b;
            a = b;
            b = tmp;
        }
        result = b;
    }

    fwrite(&result, sizeof(uint32_t), 1, stdout);
    return 0;
}
