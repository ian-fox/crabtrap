#include <stdio.h>
#include <dlfcn.h>

int main() {
    void *handle = dlopen("/usr/local/lib/libprintf_wrapper.so", RTLD_LAZY);
    if (!handle) {
        fprintf(stderr, "%s\n", dlerror());
        return 1;
    }

    int (*printf_wrapper)(const char *, ...);
    *(void **) (&printf_wrapper) = dlsym(handle, "printf_wrapper");

    char *error = dlerror();
    if (error != NULL) {
        fprintf(stderr, "%s\n", error);
        return 1;
    }

    printf("Hello from printf!\n");
    printf_wrapper("Hello from printf_wrapper!\n");

    dlclose(handle);
    return 0;
}