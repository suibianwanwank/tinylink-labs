/* 加载期的桩：编译成共享库，用 LD_PRELOAD 预加载 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include "count.h"
static void *(*real_malloc)(size_t);
static void (*real_free)(void *);
void *malloc(size_t n) {
    if (!real_malloc) real_malloc = dlsym(RTLD_NEXT, "malloc");
    n_malloc++;
    return real_malloc(n);
}
void free(void *p) {
    if (!real_free) real_free = dlsym(RTLD_NEXT, "free");
    n_free++;
    real_free(p);
}
