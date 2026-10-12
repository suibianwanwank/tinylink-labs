/* 探测 dlsym 是否在内部调用 malloc/calloc/free，并在重入时报告 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stddef.h>
#include <unistd.h>
static int in_dlsym;
static void say(const char *s) { write(2, s, __builtin_strlen(s)); }
static void *resolve(const char *name) {
    if (in_dlsym) say("  reentered while in dlsym\n");
    in_dlsym++;
    void *f = dlsym(RTLD_NEXT, name);
    in_dlsym--;
    return f;
}
void *malloc(size_t n) {
    static void *(*real)(size_t);
    if (in_dlsym) say("  malloc called from inside dlsym\n");
    if (!real) real = resolve("malloc");
    return real(n);
}
void *calloc(size_t a, size_t b) {
    static void *(*real)(size_t, size_t);
    if (in_dlsym) say("  calloc called from inside dlsym\n");
    if (!real) real = resolve("calloc");
    return real(a, b);
}
void free(void *p) {
    static void (*real)(void *);
    if (in_dlsym) say("  free called from inside dlsym\n");
    if (!real) real = resolve("free");
    real(p);
}
