#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(void) {
    char *s = strdup("hello");
    puts(s);
    free(s);
    return 0;
}
