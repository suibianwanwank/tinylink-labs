#include <stdio.h>
int parse_num(const char *s);
int main(void) { printf("parse_num(\"42\") = %d, parse_num(\"abc\") = %d\n", parse_num("42"), parse_num("abc")); return 0; }
