#include <stdio.h>
int counter = 1;
int buffer[1024];
int main(int argc, char **argv) {
    buffer[0] = counter + argc;
    printf("hello %s %d\n", argv[0], buffer[0]);
    return 0;
}
