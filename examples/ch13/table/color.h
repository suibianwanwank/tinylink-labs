/* 1.0：RED GREEN BLUE；1.1 在中间插入了 YELLOW */
enum color { RED,
#ifdef V2
             YELLOW,
#endif
             GREEN, BLUE };
const char *color_name(enum color c);
