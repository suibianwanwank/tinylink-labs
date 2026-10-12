#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
int main(void) {
    char *s = strdup("hello");
    puts(s);
    write(2, "before free\n", 12);
    free(s);
    write(2, "after free\n", 11);
    return 0;
}
