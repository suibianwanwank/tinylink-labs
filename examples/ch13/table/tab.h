/* 头文件里的内联函数：1.1 换了一个散列函数 */
#define TAB_SIZE 8
static inline unsigned tab_hash(const char *s) {
    unsigned h = 0;
#ifdef V2
    while (*s) h = (h * 33) ^ (unsigned char)*s++;
#else
    while (*s) h = h * 31 + (unsigned char)*s++;
#endif
    return h % TAB_SIZE;
}
void tab_put(const char *key, int val);   /* 库里按 tab_hash 放进槽位 */
int tab_slot(unsigned slot);              /* 读一个槽位 */
