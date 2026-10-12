// main.c：不包含头文件，三种目标都能直接编译
int printf(const char *fmt, ...);
extern int counter;          // 定义在 add.c 里
int add(int a, int b);
int main(void) {
    printf("%d\n", add(counter, 2));
    return 0;
}
