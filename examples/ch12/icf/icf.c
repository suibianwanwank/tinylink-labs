// icf.c
int add_i(int a, int b) { return a + b; }
int add_j(int a, int b) { return a + b; }
int (*pick(void))(int, int) { return add_j; }
