#include <stdio.h>
#include <stdlib.h>
__attribute__((constructor)) static void before(void) { puts("constructor: before main"); }
__attribute__((destructor))  static void after(void)  { puts("destructor: after exit"); }
static void bye(void) { puts("atexit handler"); }
int main(void) { atexit(bye); puts("main"); return 0; }
