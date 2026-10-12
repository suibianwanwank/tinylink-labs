// calls.c
static __attribute__((noinline)) int helper(int x) { return x * 3; }
__attribute__((noinline)) int pub(int x) { return x + 1; }
int count;

int caller(int x) {
    return helper(x) + pub(x) + count;
}
