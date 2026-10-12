#include "tab.h"
static int slots[TAB_SIZE];
void tab_put(const char *key, int val) { slots[tab_hash(key)] = val; }
int tab_slot(unsigned slot) { return slots[slot % TAB_SIZE]; }
