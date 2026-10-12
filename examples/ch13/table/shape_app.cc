#include <cstdio>
#include "shape.h"
int main() { Shape *s = make_square(3); std::printf("area = %d\n", s->area()); delete s; }
