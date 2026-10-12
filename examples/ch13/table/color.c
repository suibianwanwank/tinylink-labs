#include "color.h"
const char *color_name(enum color c) {
    switch (c) {
    case RED: return "red";
#ifdef V2
    case YELLOW: return "yellow";
#endif
    case GREEN: return "green";
    case BLUE: return "blue";
    }
    return "?";
}
