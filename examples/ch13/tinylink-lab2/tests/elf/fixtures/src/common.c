/* 用 -fcommon 编译：没有初始值的全局变量成为 COMMON 符号（第 3 章讲）。
   readelf 在 Value 列显示对齐，nm 在第一列显示大小。 */
int c;
int d[10] __attribute__((aligned(16)));
int defined = 1;
