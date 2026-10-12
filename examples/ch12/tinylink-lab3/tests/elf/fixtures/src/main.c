// 第 2 章的 main.c：初始化的全局变量、未初始化的数组、只读数组、指向字符串的指针。
int add(int a, int b);

int counter = 42;
int zeros[64];
const int table[4] = {1, 2, 3, 4};
const char *msg = "hello";

int main(void) {
    return add(counter, table[2]) * 2;
}
