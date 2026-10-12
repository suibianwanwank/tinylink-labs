// hotcold.c：用属性标出冷热
__attribute__((cold, noinline)) int report_error(int x) { return -x; }
__attribute__((hot, noinline)) int fast_path(int x) { return x + 1; }
__attribute__((noinline)) int normal(int x) { return x * 2; }

int main(int argc, char **argv) {
    if (argc > 5) return report_error(argc);
    return fast_path(argc) + normal(argc);
}
