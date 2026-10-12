/* 程序自己定义了 end，链接器就不再提供它；_end 仍然由链接器定义。 */
int end = 3;
extern char _end[];
char *last = _end;
int main(void) { return end; }
