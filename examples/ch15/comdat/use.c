// use.c：引用两个 COMDAT 符号
extern const int cval, aval;
int main(void) { return cval + aval; }
