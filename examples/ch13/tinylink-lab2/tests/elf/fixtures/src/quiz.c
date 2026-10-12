// 第 2 章练习一的 quiz.c。
int printf(const char *fmt, ...);
extern int limit;
extern int unused;
int total = 0;
__attribute__((weak)) int verbose = 1;
static int square(int v) { return v * v; }

int report(int n) {
    static int calls;
    int sum = square(n) + total + limit;
    if (verbose) printf("%d: %d\n", ++calls, sum);
    return sum;
}
