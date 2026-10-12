#ifndef N
#define N 1024
#endif
__attribute__((tls_model("initial-exec"))) __thread char buf[N];
char *buf_at(int i) { return &buf[i]; }
