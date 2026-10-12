// weak_b.c：g 的强定义，与 h 相同
__attribute__((noinline)) int g(int x) { return x * 5 + 1; }
