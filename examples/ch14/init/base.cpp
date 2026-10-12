#include "base.h"
#include <cstdio>

int base_value;
BaseInitializer::BaseInitializer() {
    std::puts("init base");
    base_value = 40;
}
#ifdef PRIO
__attribute__((init_priority(PRIO)))
#endif
BaseInitializer base_initializer;
