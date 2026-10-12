// handlers.c：一张函数指针表，第二项是调试用的 debug_dump
void on_read(void) {}
void debug_dump(void) {}
void (*handlers[])(void) = { on_read, debug_dump };
