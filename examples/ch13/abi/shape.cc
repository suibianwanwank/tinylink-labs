#include "shape.h"
#ifdef V2
int Shape::perimeter() const { return 4 * side; }
#endif
int Shape::area() const { return side * side; }
Shape::~Shape() {}
Shape *make_square(int side) { Shape *s = new Shape; s->side = side; return s; }
