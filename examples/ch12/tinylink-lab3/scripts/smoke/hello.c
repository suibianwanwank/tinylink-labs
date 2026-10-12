/* scripts/smoke-test.sh 用它检查容器环境：打印一行，退出码 42。 */
#include <stdio.h>

int main(int argc, char **argv) {
    printf("hello from x86-64, argc=%d, argv[1]=%s\n", argc, argc > 1 ? argv[1] : "(none)");
    return 42;
}
