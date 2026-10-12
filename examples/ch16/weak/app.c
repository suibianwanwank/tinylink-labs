// app.c：给 log_level 一个弱的默认值
#include <stdio.h>
int log_level __attribute__((weak)) = 1;
void net_init(void);
int main(void) {
    net_init();
    printf("log_level = %d\n", log_level);
    return 0;
}
