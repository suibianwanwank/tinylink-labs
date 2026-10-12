#include <stdio.h>
char small[1000];
int main(void) {
    small[0] = 1;
    FILE *f = fopen("/proc/self/maps", "r"); char l[256];
    for (int i = 0; i < 3 && fgets(l, sizeof l, f); i++) fputs(l, stdout);
    return 0;
}
