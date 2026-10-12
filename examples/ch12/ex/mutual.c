// mutual.c
int b(int x);
int a(int x) { return x ? b(x - 1) * 3 : 1; }
int b(int x) { return x ? a(x - 1) * 3 : 1; }
int c(int x) { return x ? c(x - 1) * 3 : 1; }
int d(int x) { return x ? a(x - 1) * 5 : 1; }
int main(int argc, char **argv) { return a(argc) + b(argc) + c(argc) + d(argc); }
