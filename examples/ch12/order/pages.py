#!/usr/bin/env python3
"""统计热函数（hot.txt 里的符号）覆盖了多少个 4 KiB 页、多少条 64 字节缓存行。"""
import subprocess, sys
hot = set(l.strip() for l in open("hot.txt"))
for exe in sys.argv[1:]:
    out = subprocess.run(["llvm-nm", "-S", exe], capture_output=True, text=True).stdout
    pages, lines, total = set(), set(), 0
    lo, hi = None, 0
    for l in out.splitlines():
        f = l.split()
        if len(f) == 4 and f[3] in hot:
            a, n = int(f[0], 16), int(f[1], 16)
            total += n
            lo = a if lo is None else min(lo, a); hi = max(hi, a + n)
            for x in range(a, a + n):
                pages.add(x >> 12); lines.add(x >> 6)
    print("%-10s 热代码 %d 字节，跨度 %d 字节，%d 页，%d 条缓存行" % (exe, total, hi - lo, len(pages), len(lines)))
