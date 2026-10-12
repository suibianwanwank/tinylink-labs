__attribute__((used, noinline)) static int helper(void) { return 2; }
int other(void) { return helper(); }
