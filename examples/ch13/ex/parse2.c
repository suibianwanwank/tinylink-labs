/* libparse 2：parse_num 改为输入不是数字时返回 -1，旧语义留给旧程序 */
static int digits(const char *s, int base, int *out) {
    int v = 0, n = 0;
    for (;; s++, n++) {
        int d = (*s >= '0' && *s <= '9') ? *s - '0'
              : (*s >= 'a' && *s <= 'z') ? *s - 'a' + 10 : 99;
        if (d >= base) break;
        v = v * base + d;
    }
    *out = v;
    return n;
}

/* 旧实现：名字随便起，用 .symver 挂到 parse_num@VERS_1 */
int parse_num_v1(const char *s) { int v; digits(s, 10, &v); return v; }
__asm__(".symver parse_num_v1, parse_num@VERS_1");

/* 新实现：用 GCC 10 起支持的 symver 属性挂到默认版本 */
__attribute__((symver("parse_num@@VERS_2")))
int parse_num_v2(const char *s) { int v; return digits(s, 10, &v) ? v : -1; }

/* 新函数只出现在 VERS_2 */
int parse_num_base(const char *s, int base) { int v; return digits(s, base, &v) ? v : -1; }
