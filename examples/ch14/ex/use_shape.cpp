// use_shape.cpp（练习三）：只用 Shape，不定义它的任何虚函数
#include <cstdio>
#include "shape_bad.h"
int main() {
    Shape s;
    Shape *p = &s;
    std::printf("%s %g\n", p->name(), p->area());
}
