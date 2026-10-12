// odr_b.cpp：同名的类，两个虚函数的顺序反过来，返回的字符串也不同
struct Greeter {
    virtual const char *bye() { return "bye (B)"; }
    virtual const char *hello() { return "hello (B)"; }
};
const char *call_b(Greeter *g) { return g->hello(); }
Greeter *make_b() { return new Greeter; }
