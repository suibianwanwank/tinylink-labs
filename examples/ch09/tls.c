_Thread_local int counter = 42;     /* 有初值 -> .tdata */
_Thread_local int scratch[256];     /* 无初值 -> .tbss  */
extern _Thread_local int ext_var;   /* 别的模块定义的 TLS */

int bump(void)     { return ++counter; }
int *scratch_at(int i) { return &scratch[i]; }
int read_ext(void) { return ext_var; }
