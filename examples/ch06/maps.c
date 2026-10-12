#include <stdio.h>
#include <unistd.h>
int counter = 1;
int buffer[1024];
int main(void) {
    printf("counter=%p buffer=%p sbrk(0)=%p\n", (void*)&counter, (void*)buffer, sbrk(0));
    FILE *f = fopen("/proc/self/maps", "r");
    char line[256];
    while (fgets(line, sizeof line, f)) fputs(line, stdout);
    return 0;
}
