#include <stdio.h>
int parse_num(const char *s);
int main(void) { puts("main started"); fflush(stdout); printf("parse_num(\"abc\") = %d\n", parse_num("abc")); return 0; }
