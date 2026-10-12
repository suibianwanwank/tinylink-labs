// use_ratio.cpp：声明写错了，返回类型写成 int
#include <cstdio>
#ifdef WRONG_PARAM
double ratio(long a);
#define FMT "%f"
#else
int ratio(int a);
#define FMT "%d"
#endif
int main() { std::printf("ratio(10) = " FMT "\n", ratio(10)); }
