/* "更快的 malloc"：从一块静态内存里顺序切，不支持回收 */
#include <stddef.h>
static char arena[1 << 20];
static size_t used;
void *malloc(size_t n) {
    void *p = arena + used;
    used += (n + 15) & ~(size_t)15;
    return p;
}
