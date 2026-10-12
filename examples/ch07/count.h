/* 三种桩共用：计数器，以及退出时用 write 打印（不经过 stdio） */
#include <stddef.h>
#include <stdio.h>
#include <unistd.h>
static int n_malloc, n_free;
__attribute__((destructor)) static void report(void) {
    char buf[64];
    int len = snprintf(buf, sizeof buf, "malloc=%d free=%d\n", n_malloc, n_free);
    write(2, buf, len);
}
