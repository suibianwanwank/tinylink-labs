// main.c
#include <stdio.h>

int y = 5;
int x = 7;

void set_x(void);   /* 定义在 other.c 里 */

int main(void) {
    set_x();
    printf("x = %d, y = %d\n", x, y);
    return 0;
}
