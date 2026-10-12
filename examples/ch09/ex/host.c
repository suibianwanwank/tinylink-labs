#include <dlfcn.h>
#include <stdio.h>
int main(int argc, char **argv) {
    void *h = dlopen(argv[1], RTLD_NOW);
    if (!h) { printf("dlopen: %s\n", dlerror()); return 1; }
    char *(*at)(int) = (char *(*)(int))dlsym(h, "buf_at");
    printf("ok, buf at %p\n", (void *)at(0));
    return 0;
}
