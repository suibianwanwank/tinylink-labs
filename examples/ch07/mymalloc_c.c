/* 编译期的桩：不包含上面的 malloc.h，所以这里的 malloc 是真的 */
#include <stdlib.h>
#include "count.h"
void *mymalloc(size_t n) { n_malloc++; return malloc(n); }
void myfree(void *p)     { n_free++;   free(p); }
