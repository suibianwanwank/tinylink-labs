/* fmemopen 的新旧两个实现：长度为 0 的缓冲区 */
#define _GNU_SOURCE
#include <errno.h>
#include <stdio.h>
#include <string.h>
#ifdef OLD
__asm__(".symver fmemopen, fmemopen@GLIBC_2.2.5");
#endif
int main(void) {
    char buf[1];
    FILE *f = fmemopen(buf, 0, "r");
    printf("fmemopen(buf, 0, \"r\") = %s (%s)\n", f ? "FILE*" : "NULL", f ? "ok" : strerror(errno));
    return 0;
}
