// weak_a.c：g 在这里是弱定义，强定义在 weak_b.c
__attribute__((weak, noinline)) int g(int x) { return x * 9; }
__attribute__((noinline)) int h(int x) { return x * 5 + 1; }
__attribute__((noinline)) int f1(int x) { return g(x) + 2; }
__attribute__((noinline)) int f2(int x) { return h(x) + 2; }
int main(int argc, char **argv) { return f1(argc) + f2(argc); }
