#include <stdio.h>

int printf_wrapper(const char *format, ...);

int main() {
    printf("Hello from printf!\n");
    printf_wrapper("Hello from printf_wrapper!\n");
    return 0;
}