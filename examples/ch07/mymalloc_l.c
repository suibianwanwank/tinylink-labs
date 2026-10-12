/* 链接期的桩：配合 -Wl,--wrap=malloc,--wrap=free */
#include "count.h"
void *__real_malloc(size_t n);
void __real_free(void *p);
void *__wrap_malloc(size_t n) { n_malloc++; return __real_malloc(n); }
void __wrap_free(void *p)     { n_free++;   __real_free(p); }
