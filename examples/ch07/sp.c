static int table[4] = { 10, 20, 30, 40 };
int *cursor = &table[2];          /* 需要按基址修正的绝对地址 */
int main(void) { return *cursor; }
