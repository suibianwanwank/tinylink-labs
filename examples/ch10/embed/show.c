#define UART 0x10000000UL
static void putc(char c) {
    volatile unsigned char *u = (unsigned char *)UART;
    while ((u[5] & 0x20) == 0) ;
    u[0] = c;
}
extern const char _binary_data_msg_v1_txt_start[], _binary_data_msg_v1_txt_end[];
extern const char _binary_data_msg_v1_txt_size[];   /* 绝对符号：它的"地址"就是长度 */
void main(void) {
    for (const char *p = _binary_data_msg_v1_txt_start; p < _binary_data_msg_v1_txt_end; p++)
        putc(*p);
    unsigned long n = (unsigned long)_binary_data_msg_v1_txt_size;
    putc('0' + n / 10); putc('0' + n % 10); putc('\n');
}
