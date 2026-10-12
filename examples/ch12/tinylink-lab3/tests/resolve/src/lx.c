/* 和 gx.c 里的全局 x 同名的 static 变量：它是局部符号，不参加解析。 */
__attribute__((used)) static int x = 2;
int peek(void) { return x; }
