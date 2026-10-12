// comdat_a.cpp：两个文件都实例化了同一个模板函数
template <class T> __attribute__((noinline)) T twice(T x) { return x + x; }
int use_a(int x) { return twice(x); }
