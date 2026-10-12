// use_shape.cpp：只用 Shape，不定义 area()
#include <cstdio>
#include "shape.h"
int main() {
    Shape s;
    Shape *p = &s;
    std::printf("%s %g\n", p->name(), p->area());
}
