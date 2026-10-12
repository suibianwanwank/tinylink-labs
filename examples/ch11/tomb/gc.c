int counter = 1;

__attribute__((noinline)) int used(int x) {
    return x + counter;
}

__attribute__((noinline)) int unused(int x) {
    return x * 3 + counter;
}

void _start(void) {
    int r = used(41);
    __asm__ volatile("mov %0, %%edi; mov $60, %%eax; syscall" :: "r"(r) : "rdi", "rax");
}
