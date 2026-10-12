// area.cpp：C++ 写的实现，给 C 程序调用
#ifdef WITH_EXTERN_C
extern "C"
#endif
int area(int w, int h) { return w * h; }
