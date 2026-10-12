#include <stdio.h>
static int add(int a, int b) { return a + b; }
static int sub(int a, int b) { return a - b; }
int (*const ops[])(int, int) = { add, sub };   /* 数据里存的绝对地址 */
const char *greeting = "hi";
int main(void) {
    printf("ops[0]=%p ops[1]=%p greeting=%p main=%p -> %d %s\n",
           (void *)ops[0], (void *)ops[1], (void *)greeting, (void *)main,
           ops[0](40, 2), greeting);
    return 0;
}
