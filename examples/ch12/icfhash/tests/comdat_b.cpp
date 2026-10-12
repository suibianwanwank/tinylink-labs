// comdat_b.cpp
template <class T> __attribute__((noinline)) T twice(T x) { return x + x; }
int use_b(int x) { return twice(x); }
int use_a(int);
int main(int argc, char **) { return use_a(argc) + use_b(argc); }
