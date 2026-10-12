// handcalc.cpp：练习二的两个声明，用来核对手算的修饰名
namespace geo { struct Point { int x, y; }; void scale(Point *, const Point *, double) {} }
namespace shape { struct circle { void resize(const circle &, double); }; void circle::resize(const circle &, double) {} }
