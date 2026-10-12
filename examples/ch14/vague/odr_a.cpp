// odr_a.cpp：这一份 Greeter 先声明 hello 再声明 bye
struct Greeter {
    virtual const char *hello() { return "hello (A)"; }
    virtual const char *bye() { return "bye (A)"; }
};
const char *call_a(Greeter *g) { return g->hello(); }
Greeter *make_a() { return new Greeter; }
