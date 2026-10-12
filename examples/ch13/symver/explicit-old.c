#include <stdio.h>
extern int parse_old(const char *s);
__asm__(".symver parse_old,parse_num@VERS_1");
int main(void) {
    printf("explicit old: %d\n", parse_old("abc"));
    return 0;
}
