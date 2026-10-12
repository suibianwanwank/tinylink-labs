/* libpoint 1.1 的头文件：结构体末尾加了一个字段 */
struct point { int x; int y; int z; };
void point_init(struct point *p, int x, int y);
int point_sum(const struct point *p);
