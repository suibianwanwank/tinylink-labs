#!/usr/bin/env python3
"""生成节顺序实验的程序：N 个小函数，每隔 STRIDE 个取一个当热函数。
用法：gen.py N STRIDE > prog.c；同时写出 hot.txt（热函数的符号清单）。"""
import sys
n, stride = int(sys.argv[1]), int(sys.argv[2])
out = []
out.append("// 由 gen.py 生成：%d 个函数，每 %d 个里有 1 个热函数\n" % (n, stride))
for i in range(n):
    a, b, c = 2654435761 * (i + 1) & 0xffffffff, (i % 13) + 3, (i * 40503) & 0xffff
    out.append(
        "__attribute__((noinline)) unsigned f%d(unsigned x) {\n"
        "    x = (x ^ (x >> %d)) * %du;\n"
        "    return x + %du;\n"
        "}\n" % (i, b, a | 1, c))
hot = [i for i in range(0, n, stride)]
out.append("__attribute__((noinline)) unsigned run(unsigned x) {\n")
for i in hot:
    out.append("    x = f%d(x);\n" % i)
out.append("    return x;\n}\n")
out.append("__attribute__((noinline, cold)) unsigned cold_path(unsigned x) {\n")
for i in range(n):
    if i % stride:
        out.append("    x = f%d(x);\n" % i)
out.append("    return x;\n}\n")
out.append(
    "int main(int argc, char **argv) {\n"
    "    unsigned x = argc;\n"
    "    if (argc > 100) x = cold_path(x);\n"
    "    for (int i = 0; i < ITER; i++) x = run(x);\n"
    "    return x & 0x7f;\n"
    "}\n")
sys.stdout.write("".join(out))
with open("hot.txt", "w") as f:
    f.write("main\nrun\n" + "".join("f%d\n" % i for i in hot))
