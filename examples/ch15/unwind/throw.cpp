// throw.cpp：带 try/catch 的函数需要 personality 和 LSDA
extern "C" int printf(const char *, ...);
extern "C" int odd(void);
static void thrower(int x) { if (x) throw x; }
int main(int argc, char **) {
    try { thrower(argc); } catch (int v) { printf("caught %d, odd=%d\n", v, odd()); }
    return 0;
}
