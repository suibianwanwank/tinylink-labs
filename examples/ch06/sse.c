typedef int v4 __attribute__((vector_size(16)));
v4 g = { 41, 1, 0, 0 };
__attribute__((noinline)) int f(void) { volatile v4 x = g; return x[0] + x[1]; }
__attribute__((noreturn)) void start_c(void) {
    __asm__ volatile("syscall" :: "a"(60), "D"(f()));
    __builtin_unreachable();
}
