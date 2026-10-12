// gc.c
int ext_counter;
__attribute__((noinline)) int live(int x) { ext_counter += x; return ext_counter * 3; }
__attribute__((noinline)) int dead(int x) { ext_counter -= x; return ext_counter * 5; }
void _start(void) { live(1); for (;;) {} }
