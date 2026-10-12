#include <stdio.h>
#include <string.h>
#include <unistd.h>

#define N (32u << 20)                       /* 32 MiB */
const unsigned char table[N] = { 1 };       /* 进 .rodata，占文件空间 */
unsigned char zeros[N];                     /* 进 .bss，不占文件空间 */

static void show(const char *when) {
    FILE *f = fopen("/proc/self/smaps", "r");
    char line[256], range[64] = "", perm[8] = "";
    unsigned long size = 0, kb;
    while (fgets(line, sizeof line, f)) {
        if (sscanf(line, "%63[0-9a-f]-%*s %7s", range, perm) == 2) continue;
        if (sscanf(line, "Size: %lu", &kb) == 1) size = kb;
        if (sscanf(line, "Rss: %lu", &kb) == 1 && size >= 1024)
            printf("%-12s %s %s Size %6lu kB  Rss %6lu kB\n", when, range, perm, size, kb);
    }
    fclose(f);
}

int main(void) {
    printf("table=%p zeros=%p\n", (void *)table, (void *)zeros);
    show("start");
    unsigned long sum = 0;
    for (unsigned i = 0; i < N; i += 4096) sum += table[i];
    show("after-table");
    for (unsigned i = 0; i < N; i += 4096) zeros[i] = 1;
    show("after-zeros");
    return (int)(sum & 1) ^ 1;
}
