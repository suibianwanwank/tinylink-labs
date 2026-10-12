// a.c → liba.dylib
int puts(const char *);
void hello(void) { puts("hello from liba"); }
#ifndef NO_ONLY_A
void only_a(void) { puts("only_a"); }
#endif
