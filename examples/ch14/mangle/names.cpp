// names.cpp：只看修饰名，函数体都是空的
#include <string>
namespace geo {
struct Point { int x, y; int norm() const; };
int Point::norm() const { return x * x + y * y; }
double dist(const Point &a, const Point &b) { return 0; }
void move(Point &p, const Point &d) {}
}
void report(int) {}
void report(double) {}
void report(const char *) {}
void report(const std::string &) {}
template <typename T> T biggest(T a, T b) { return a > b ? a : b; }
template long biggest<long>(long, long);
template double biggest<double>(double, double);
extern "C" void report_c(int) {}
int counter = 0;
namespace geo { int counter = 0; }
