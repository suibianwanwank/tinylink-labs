#define UART 0x10000000UL
static void putc(char c) {
    volatile unsigned char *u = (unsigned char *)UART;
    while ((u[5] & 0x20) == 0) ;          /* LSR.THRE：发送缓冲空 */
    u[0] = c;
}
static void puts(const char *s) { while (*s) putc(*s++); }

const char *greeting = "2: via pointer in .data\n";   /* .data 里存着一个绝对地址 */
static void bye(void) { puts("3: via function pointer\n"); }
void (*hook)(void) = bye;                               /* 同上，存的是函数地址 */

void main(void) {
    puts("1: via pc-relative literal\n");
    puts(greeting);
    hook();
    puts("4: done\n");
}
