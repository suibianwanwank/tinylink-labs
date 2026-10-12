#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <malloc.h>

char *greet(void);              /* 在静态库 libgreet.a 里 */

int main(void) {
    char *p = malloc(10);       /* 自己的代码 */
    char *s = strdup("hi");     /* libc 内部调用 malloc */
    char *g = greet();          /* 静态库里调用 malloc */
    printf("%s %s\n", s, g);
    free(p); free(s); free(g);
    return 0;
}
