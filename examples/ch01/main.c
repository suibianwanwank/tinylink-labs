// main.c
int add(int a, int b);   // 只声明，定义在 add.c 里
int counter = 1;
int main(void) { return add(counter, 2); }
