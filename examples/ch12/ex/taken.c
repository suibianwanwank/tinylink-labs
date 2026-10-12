// taken.c
typedef int (*fn)(int);
int h1(int x) { return x ^ 0x55; }
int h2(int x) { return x ^ 0x55; }
int h3(int x) { return x ^ 0x55; }
int h4(int x) { return x ^ 0x55; }
fn table[] = { h2, h4 };
int main(int argc, char **argv) { return h1(argc) + h3(argc) + table[argc & 1](argc); }
