// b.c → libb.dylib：也定义了 hello
int puts(const char *);
void hello(void) { puts("hello from libb"); }
