#include <stdio.h>
#include <string.h>
static int add(int a, int b) { return a + b; }
int (*const ops[])(int, int) = { add };
int main(int argc, char **argv) {
    printf("ops[0]=%p main=%p\n", (void *)ops[0], (void *)main);
    const char *me = strrchr(argv[0], '/') ? strrchr(argv[0], '/') + 1 : argv[0];
    FILE *f = fopen("/proc/self/maps", "r"); char l[256];
    while (fgets(l, sizeof l, f)) if (strstr(l, me) || strstr(l, "heap")) fputs(l, stdout);
    return 0;
}
