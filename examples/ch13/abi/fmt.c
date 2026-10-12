/* 对外只承诺 fmt_width；fmt_pad 是内部函数，2 版改了它的参数 */
#ifdef V2
int fmt_pad(int n, char c) { return c == ' ' ? n : 0; }
#define PAD(n) fmt_pad(n, ' ')
#else
int fmt_pad(int n) { return n; }
#define PAD(n) fmt_pad(n)
#endif
#ifdef EXPORT_MACRO
#define FMT_API __attribute__((visibility("default")))
#else
#define FMT_API
#endif
FMT_API int fmt_width(int len, int width) { return len < width ? PAD(width - len) : 0; }
