// tget2.cpp：这里只看得到第二个模板，返回的是指针
template <typename T> T *get(int) { static T v = 9; return &v; }
int use2() { return *get<int>(2); }
