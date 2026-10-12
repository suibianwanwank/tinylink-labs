// shape.cpp（练习三）：定义了 area；加 -DFIX 时也定义 draw
#include "shape_bad.h"
double Shape::area() const { return 0; }
#ifdef FIX
void Shape::draw() const {}
#endif
