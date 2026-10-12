typedef void (*initcall_t)(void);
#define initcall(fn, lvl) \
    static initcall_t __initcall_##fn __attribute__((used, section(".initcall" #lvl))) = fn
static void a(void) {}  initcall(a, 3);
static void b(void) {}  initcall(b, 1);
static void c(void) {}  initcall(c, 2);
extern initcall_t __initcall_start[], __initcall_end[];
void _entry(void) { for (initcall_t *p = __initcall_start; p < __initcall_end; p++) (*p)(); }
