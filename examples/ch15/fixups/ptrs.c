// ptrs.c：数据段里放几个指针，看链接器为它们留下什么
int printf(const char *fmt, ...);
void *malloc(unsigned long n);
int counter = 40;
int *p_counter = &counter;                 // 指向本模块：需要 rebase
void *p_printf = (void *)&printf;          // 指向 libSystem：需要 bind
void *p_malloc = (void *)&malloc;          // 同上
int main(void) { return *p_counter + (p_printf != 0) + (p_malloc != 0); }
