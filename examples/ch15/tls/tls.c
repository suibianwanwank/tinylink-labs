// tls.c：一个线程局部变量
_Thread_local int t = 5;
int get(void) { return t; }
