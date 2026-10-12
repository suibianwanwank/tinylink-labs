// b.cpp
#include <cstdio>
#include "common.h"
int fa(int x);
int fb(int x) { return twice(x) + add<int>(x, 2) + next_id(); }
int main() {
    int a = fa(1), b = fb(1);
    std::printf("fa=%d fb=%d next_id=%d\n", a, b, next_id());
}
