#include <stdio.h>
int main(int argc, char **argv) {
    printf("hello from Linux, argc=%d, arg=%s\n", argc,
           argc > 1 ? argv[1] : "(none)");
    return 42;
}
