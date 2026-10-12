#include <stdio.h>
#include "tab.h"
int main(void) {
    tab_put("apple", 7);
    printf("apple -> slot %u -> %d\n", tab_hash("apple"), tab_slot(tab_hash("apple")));
    return 0;
}
