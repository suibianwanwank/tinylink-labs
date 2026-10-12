/* 只引用、不定义：这些名字由链接器合成。 */
extern char __ehdr_start[], etext[], edata[], __bss_start[], end[];
char *marks[] = {__ehdr_start, etext, edata, __bss_start, end};
int main(void) { return 0; }
