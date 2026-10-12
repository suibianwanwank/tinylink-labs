// 和 shape_app.cc 一样调用 area() 再 delete s，但替换了全局 operator new/delete 来计数。
// 程序里定义的 operator delete 会插入（第 7 章）到 libshape.so.1 对它的调用上。
#include <cstdio>
#include <cstdlib>
#include <new>
#include "shape.h"
static int news, deletes;
void *operator new(std::size_t n) { ++news; return std::malloc(n); }
void operator delete(void *p) noexcept { ++deletes; std::free(p); }
void operator delete(void *p, std::size_t) noexcept { ++deletes; std::free(p); }
int main() {
    Shape *s = make_square(3);
    int a = s->area();
    delete s;
    std::printf("area = %d, operator new %d, operator delete %d\n", a, news, deletes);
}
