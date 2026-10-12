#include <stdio.h>
#include POINT_H
struct point pts[3];     /* 第三个留作备用 */
int main(void) {
    point_init(&pts[0], 1, 2);
    point_init(&pts[1], 10, 20);
    printf("sizeof(struct point) = %zu\n", sizeof(struct point));
    printf("sum0 = %d, sum1 = %d\n", point_sum(&pts[0]), point_sum(&pts[1]));
    return 0;
}
