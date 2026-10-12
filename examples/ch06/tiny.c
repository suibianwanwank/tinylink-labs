#include <stdio.h>
int counter = 1;
int buffer[1024];
int main(void) { buffer[0] = counter; puts("ok"); return 0; }
