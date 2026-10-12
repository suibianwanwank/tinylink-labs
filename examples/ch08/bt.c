// bt.c
#include <execinfo.h>
#include <unistd.h>

static __attribute__((noinline)) void report(void) {
    void *pcs[16];
    int n = backtrace(pcs, 16);
    backtrace_symbols_fd(pcs, n, STDOUT_FILENO);
}
__attribute__((noinline)) void leaf(void)   { report(); }
__attribute__((noinline)) void middle(void) { leaf(); }
int main(void) { middle(); return 0; }
