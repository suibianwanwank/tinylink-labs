// odr_main.cpp：两边各造一个对象，各自按自己的类定义调用 hello()
#include <cstdio>
struct Greeter;
const char *call_a(Greeter *); Greeter *make_a();
const char *call_b(Greeter *); Greeter *make_b();
int main() {
    std::printf("call_a(make_a()) = %s\n", call_a(make_a()));
    std::printf("call_b(make_b()) = %s\n", call_b(make_b()));
}
