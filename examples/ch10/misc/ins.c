__attribute__((used, section(".mytable"))) static const int t[2] = {1, 2};
extern const int __tbl_start[], __tbl_end[];
int _start(void) { return __tbl_end - __tbl_start; }
