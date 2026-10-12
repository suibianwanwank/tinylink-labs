#include "base_fix.h"
#include <cstdio>

static int make_base() {
    std::puts("init base");
    return 40;
}
int &base() {
    static int value = make_base();
    return value;
}
