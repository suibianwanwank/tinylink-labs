// kinds.c：arm64 上几种常见的访问方式
extern int table[16] __attribute__((visibility("hidden")));  // 链接时一定在本模块里
extern int ext_var;          // 可能来自 dylib：走 GOT
void callee(void);
int *p_elem = &table[5];                         // 数据里的指针：ARM64_RELOC_UNSIGNED
int *addr_of_elem(void) { return &table[3]; }   // 地址加常量：要带加数
int  read_ext(void)     { return ext_var; }
void call_it(void)      { callee(); }           // 尾调用：BRANCH26
