// chain.c：字节相同，调用的对象不同
int g1(int x) { return x * 5 + 1; }
int g2(int x) { return x * 5 + 1; }    // 与 g1 相同
int g3(int x) { return x * 7 + 1; }    // 与 g1 不同
int f1(int x) { return g1(x) + 2; }
int f2(int x) { return g2(x) + 2; }
int f3(int x) { return g3(x) + 2; }
int main(int argc, char **argv) { return f1(argc) + f2(argc) + f3(argc); }
