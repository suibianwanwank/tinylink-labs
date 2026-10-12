/* 3 版：只留新实现 */
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
int parse_num(const char *s) { int v; return digits(s, 10, &v) ? v : -1; }
int parse_num_base(const char *s, int base) { int v; return digits(s, base, &v) ? v : -1; }
