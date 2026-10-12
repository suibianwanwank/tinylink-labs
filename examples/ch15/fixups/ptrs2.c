// ptrs2.c：练习二用，先预测链上每个指针的原始值
int printf(const char *fmt, ...);
int counter = 40;
int *p_counter = &counter;
int *p_next = &counter + 1;                // 指向 counter 之后 4 字节
void *p_printf = (void *)&printf;
int main(void) { return *p_counter + (int)(p_next - p_counter) + (p_printf != 0); }
