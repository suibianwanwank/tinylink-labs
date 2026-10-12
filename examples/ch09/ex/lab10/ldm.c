static _Thread_local int a = 1;
static _Thread_local int b;
int touch(int x) { a += x; b -= x; return a + b; }
int main(void) { return touch(1); }
