/* 两个文件各有一个 static 的 helper：局部符号不进全局表，不会撞车。 */
__attribute__((used, noinline)) static int helper(void) { return 1; }
int main(void) { return helper(); }
