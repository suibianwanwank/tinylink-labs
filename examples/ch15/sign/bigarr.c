// bigarr.c：256KB 的已初始化数组，按命令行参数只读其中一个元素
#include <stdlib.h>
int arr[65536] = {1};                 // 放在 __DATA,__data，全部内容都在文件里
int main(int argc, char **argv) {
    if (argc < 2) return 0;           // 不带参数：一页都不碰
    volatile int v = arr[atoi(argv[1])];
    (void)v;
    return 0;
}
