// asm_main.c：只在内联汇编里调用 helper2
int main(void) {
    int r;
    __asm__ volatile("call helper2" : "=a"(r) : : "rdi", "rsi", "rdx", "rcx", "r8", "r9", "r10", "r11", "memory");
    return r;
}
