// ptr.c：折叠以后，两个不同函数的地址还不相等吗？
int add_i(int a, int b);
int (*pick(void))(int, int);

int main(void) {
    int (*p)(int, int) = pick();          // pick 返回 add_j
    return (p == add_i) * 10 + p(1, 2);   // 不折叠应得 3
}
