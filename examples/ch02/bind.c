// bind.c
static int hits;                         /* 只在本文件可见 */
__attribute__((weak)) int hook(void) {   /* 弱定义：别人可以覆盖 */
    return 0;
}
extern int maybe(void) __attribute__((weak));  /* 弱引用 */

int poke(void) {
    hits++;
    return hook() + (maybe ? maybe() : 0) + hits;
}
