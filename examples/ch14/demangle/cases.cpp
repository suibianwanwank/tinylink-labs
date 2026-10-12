// cases.cpp：对拍用的声明，只用命名空间、类、内建类型、指针、引用和 const
namespace a {
struct D {};
namespace b {
struct C {
    int f(int) const;
    void g(C *, const C &);
    static long counter;
};
int C::f(int) const { return 0; }
void C::g(C *, const C &) {}
long C::counter = 0;
}  // namespace b
void k(D, D *, D &, const D *, b::C, b::C *) {}
}  // namespace a
void h1(a::D, a::D *, a::b::C &, const a::b::C *) {}
void h2(unsigned long, long long, unsigned char, signed char, bool, float, double,
        long double, short, unsigned short, unsigned int, char **, const char *const *) {}
void h3(a::b::C **, a::b::C *const *, const a::b::C &, a::b::C *const &) {}
void h4(wchar_t, __int128, unsigned __int128, unsigned long long, ...) {}
void h5() {}
namespace a { namespace b { void deep(C, b::C *, a::D *, const a::D &) {} } }
// 特殊名字：vtable、typeinfo、guard 变量、局部实体
namespace a {
struct V {
    virtual int f() const;
    virtual ~V();
};
int V::f() const { static int calls = 0; return ++calls; }
V::~V() {}
}  // namespace a
int &cfg() { static int x = a::b::C::counter > 0 ? 1 : 2; return x; }
int count_k(const a::D &) { static long seen; return ++seen; }
