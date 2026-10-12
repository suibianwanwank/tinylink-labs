/* 练习三的修法：把 malloc 一族一起换掉 */
#include <stddef.h>
#include <string.h>
static char arena[1 << 20];
static size_t used;
void *malloc(size_t n) {
    void *p = arena + used;
    used += (n + 15) & ~(size_t)15;
    return p;
}
void free(void *p) { (void)p; }          /* 不回收 */
void *calloc(size_t a, size_t b) { return memset(malloc(a * b), 0, a * b); }
void *realloc(void *old, size_t n) {
    void *p = malloc(n);
    if (old) memcpy(p, old, n);           /* 示意：可能多读，arena 足够大 */
    return p;
}
