// tget1.cpp：这里只看得到第一个模板
template <typename T> T get(int) { return T(7); }
int use1() { return get<int>(1); }
