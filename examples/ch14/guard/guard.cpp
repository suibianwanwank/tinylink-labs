// guard.cpp：函数内的 static，初值要到第一次调用时才算
#include <cstdio>
static int compute() { std::puts("compute"); return 42; }

int &config() {
    static int value = compute();
    return value;
}

inline int &config_inl() {          // inline 版本：value 和 guard 都要走 vague linkage
    static int value = compute();
    return value;
}
int use_inl() { return config_inl(); }
