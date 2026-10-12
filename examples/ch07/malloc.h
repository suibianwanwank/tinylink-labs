/* 编译期打桩：用 -I. 让它挡在系统的 malloc.h 前面 */
#include <stddef.h>
#define malloc(n) mymalloc(n)
#define free(p)   myfree(p)
void *mymalloc(size_t n);
void myfree(void *p);
