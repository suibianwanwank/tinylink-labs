"""从一次真实链接的结果里摘出符号解析的结论，格式见 tests/resolve_vs_linkers.rs 开头。

用法：verdict.py 工作目录 lld|gnu 退出码 readelf 输入文件...
工作目录里有链接器的 stderr、映射文件 out.map 和产物 out.elf（链接成功时）。
"""

import re
import subprocess
import sys

work, which, status, readelf = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
objs = sys.argv[5:]


def symbols(path):
    """readelf -sW 的每一行：(名字, 值, 大小, 绑定, 可见性, 节)。"""
    out = subprocess.run([readelf, "-sW", path], cwd=work, capture_output=True, text=True).stdout
    for line in out.splitlines():
        cols = line.split()
        if len(cols) == 8 and cols[0].endswith(":") and cols[0][:-1].isdigit():
            _, value, size, _, bind, vis, ndx, name = cols
            yield name, int(value, 16), int(size, 0), bind, vis, ndx


lines = [f"status {0 if status == 0 else 1}"]

# 报了哪些错。lld 对 hidden、protected 的未定义符号写 "undefined hidden symbol"。
err = open(f"{work}/stderr").read()
if which == "lld":
    pats = [(r"error: duplicate symbol: (\S+)", "duplicate"),
            (r"error: undefined (?:hidden |protected |internal )?symbol: (\S+)", "undefined")]
else:
    pats = [(r"multiple definition of `([^']+)'", "duplicate"),
            (r"undefined reference to `([^']+)'", "undefined"),
            (r"hidden symbol `([^']+)' isn't defined", "undefined")]
errors = set()
for pat, kind in pats:
    for m in re.finditer(pat, err):
        errors.add(f"error {kind} {m.group(1)}")
lines += sorted(errors)

if status == 0:
    # 输入里出现过的全局名字。
    names = set()
    statics = set()
    for o in objs:
        for name, _, _, bind, _, ndx in symbols(o):
            if bind in ("GLOBAL", "WEAK"):
                names.add(name)
            elif ndx != "UND":
                statics.add(name)

    def is_static(name, bind, vis):
        """产物里的一个局部符号是不是输入里某个 static 变量、函数的那一份。
        lld 会把 hidden 符号降成 LOCAL；链接器合成的 __ehdr_start 也可能是 LOCAL。"""
        return bind == "LOCAL" and vis == "DEFAULT" and name in statics

    # 产物里这些名字的地址、大小和所属节。输入里同名的 static 变量的那一份不算。
    output = {}
    for name, value, size, bind, vis, ndx in symbols("out.elf"):
        if name in names and not is_static(name, bind, vis):
            output[name] = (value, size, ndx)

    # 映射文件：一个地址等于产物里那个全局符号的符号行，它前面最近的输入节就是定义所在。
    # 只看地址，同名的 static 变量（地址不同）就不会混进来。
    winners = {}
    text = open(f"{work}/out.map").read().splitlines()
    if which == "lld":
        # 表头里 Out、In、Symbol 三列的起点：输出节、输入节、符号各占一列，靠缩进区分。
        out_col = text[0].index("Out")
        in_col = text[0].index("In") - out_col
        sym_col = text[0].index("Symbol") - out_col
        current = None
        for line in text[1:]:
            rest = line[out_col:]
            indent = len(rest) - len(rest.lstrip())
            item = rest.strip()
            cols = line[:out_col].split()
            m = re.match(r"(.+?):\((.+?)(\+0x[0-9a-f]+)?\)$", item)
            if indent == in_col and m:
                # VMA LMA Size Align：COMMON 的输入节就是这一个符号，Align 列是它的对齐。
                current = (m.group(1), m.group(2), int(cols[3]))
            elif indent == sym_col and current and item in output and int(cols[0], 16) == output[item][0]:
                winners[item] = current
    else:
        current = None
        pending = None
        for line in text:
            m = re.match(r"^ (\S+)\s+0x[0-9a-f]+\s+0x[0-9a-f]+ (\S+)$", line)
            if m:
                current = (m.group(2), m.group(1), None)
                continue
            m = re.match(r"^ (\S+)$", line)
            if m:
                pending = m.group(1)
                continue
            m = re.match(r"^\s+0x[0-9a-f]+\s+0x[0-9a-f]+ (\S+)$", line)
            if m and pending:
                current = (m.group(1), pending, None)
                pending = None
                continue
            m = re.match(r"^\s{16}(0x[0-9a-f]+)\s+([A-Za-z_.$][\w.$]*)$", line)
            if m and current and m.group(2) in output and int(m.group(1), 16) == output[m.group(2)][0]:
                winners[m.group(2)] = current

    result = {}
    for name, (file, section, align) in winners.items():
        size = output[name][1]
        if section == "COMMON":
            # GNU ld 的映射文件不写对齐，比较时这一项不算（"?"）。
            result[name] = f"{file} COMMON size {size} align {'?' if align is None else align}"
        else:
            result[name] = f"{file} {section} size {size}"
    for name, value, size, bind, vis, ndx in symbols("out.elf"):
        if name not in names or name in result:
            continue
        if is_static(name, bind, vis):
            continue
        if ndx == "ABS":
            result[name] = f"*ABS* {value:#x}"
        elif ndx == "UND":
            result[name] = "UNDEF-WEAK" if bind == "WEAK" else "UNDEF"
        else:
            result[name] = "SYNTHETIC"
    lines += [f"{name} {where}" for name, where in sorted(result.items())]
print("\n".join(lines))
