#include POINT_H
void point_init(struct point *p, int x, int y) {
    p->x = x;
    p->y = y;
#ifdef HAVE_Z
    p->z = 0;            /* 新字段默认为 0 */
#endif
}
int point_sum(const struct point *p) {
#ifdef HAVE_Z
    return p->x + p->y + p->z;
#else
    return p->x + p->y;
#endif
}
