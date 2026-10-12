// a.cpp
#include "common.h"
int fa(int x) { return twice(x) + add<int>(x, 1) + next_id(); }
