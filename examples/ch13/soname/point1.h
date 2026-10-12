/* libpoint 1.0 的头文件 */
struct point { int x; int y; };
void point_init(struct point *p, int x, int y);
int point_sum(const struct point *p);
