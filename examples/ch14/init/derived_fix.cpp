#include "base_fix.h"
#include <cstdio>

static int make_derived() {
    std::puts("init derived");
    return base() + 2;
}
int derived = make_derived();
