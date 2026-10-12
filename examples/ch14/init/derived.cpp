#include "base.h"
#include <cstdio>

static int make_derived() {
    std::puts("init derived");
    return base_value + 2;
}
int derived = make_derived();
