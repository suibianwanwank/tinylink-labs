#!/usr/bin/env python3
"""往符号解析的代码里逐个埋 bug，看测试能不能抓住（实验 3"测试能不能抓住有 bug 的实现"一节）。

每个 bug 是对源文件的一处文本替换。脚本依次：埋下一个 bug，运行 `cargo test --no-fail-fast`，
记下哪几层测试失败，再把文件恢复原样。最后打印一张表。

用法：scripts/plant-bugs.py            跑全部 bug
      scripts/plant-bugs.py B3 B7      只跑这几个
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
TABLE = "src/resolve/table.rs"
MOD = "src/resolve/mod.rs"

BUGS = [
    ("B1", "COMMON 的大小取后来的", TABLE,
     "size: size.max(old_size),", "size,"),
    ("B2", "COMMON 的对齐取后来的", TABLE,
     "align: align.max(old_align),", "align,"),
    ("B3", "COMMON 归后来的文件", TABLE,
     "at: if size > old_size { at } else { old_at },", "at: if size >= old_size { at } else { old_at },"),
    ("B4", "可见性取后来的", TABLE,
     "if strictness(symbol.visibility) > strictness(entry.visibility) {", "if true {"),
    ("B5", "两个弱定义取后来的", TABLE,
     "(_, Incoming::Defined) if weak => None,",
     "(Definition::Defined { .. }, Incoming::Defined) if weak => {\n"
     "                entry.definition = defined;\n                None\n            }\n"
     "            (_, Incoming::Defined) if weak => None,"),
    ("B6", "强定义不替换弱定义", TABLE,
     "            ) => {\n                entry.definition = defined;\n                None\n            }\n"
     "            (Definition::Common { .. }, Incoming::Defined) => {",
     "            ) => None,\n"
     "            (Definition::Common { .. }, Incoming::Defined) => {"),
    ("B7", "强定义不替换 COMMON", TABLE,
     "            (Definition::Common { .. }, Incoming::Defined) => {\n                entry.definition = defined;\n                None\n            }",
     "            (Definition::Common { .. }, Incoming::Defined) => None,"),
    ("B8", "COMMON 不替换弱定义", TABLE,
     "                if binding == SymbolBinding::Weak {\n                    entry.definition = Definition::Common { at, size, align };\n                }",
     "                let _ = (binding, size, align);"),
    ("B9", "弱定义替换已有的 COMMON", TABLE,
     "(_, Incoming::Defined) if weak => None,",
     "(Definition::Common { .. }, Incoming::Defined) if weak => {\n"
     "                entry.definition = defined;\n                None\n            }\n"
     "            (_, Incoming::Defined) if weak => None,"),
    ("B10", "不报重复定义", TABLE,
     "(!same_absolute).then_some(Duplicate {", "(same_absolute && !same_absolute).then_some(Duplicate {"),
    ("B11", "值相同的绝对符号也报重复定义", TABLE,
     "let same_absolute = section == SectionIndex::Abs", "let same_absolute = false && section == SectionIndex::Abs"),
    ("B12", "合成符号覆盖真实的定义", TABLE,
     "            if entry.definition == Definition::Undefined\n", "            if true\n"),
    ("B13", "局部符号进全局表", MOD,
     "SymbolBinding::Local => continue,", "SymbolBinding::Local => {}"),
    ("B14", "按引用自己的绑定判断要不要报未定义", MOD,
     "if allocated && entry.definition == Definition::Undefined && entry.strong_reference",
     "if allocated\n                    && entry.definition == Definition::Undefined\n"
     "                    && input.symbols[r.rela.symbol as usize].binding != SymbolBinding::Weak"),
    ("B15", "只有弱引用也报未定义", MOD,
     "if allocated && entry.definition == Definition::Undefined && entry.strong_reference",
     "if allocated && entry.definition == Definition::Undefined"),
    ("B16", "不可分配节里的重定位也算引用", MOD,
     "if allocated && entry.definition == Definition::Undefined && entry.strong_reference",
     "if entry.definition == Definition::Undefined && entry.strong_reference && (allocated || true)"),
]

# cargo test 的每个测试二进制对应一层。
LAYERS = [
    ("unittests src/lib.rs", "单元"),
    ("tests/cases.rs", "golden/errors"),
    ("tests/resolve_vs_linkers.rs", "对拍"),
    ("tests/elf_malformed.rs", "畸形输入"),
]


def run_tests():
    # stderr 并进 stdout：cargo 的 "Running ..." 写在 stderr，测试结果写在 stdout，分开就对不上了。
    out = subprocess.run(
        ["cargo", "test", "--no-fail-fast"],
        cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
    )
    text = out.stdout
    caught = []
    if "error[E" in text:
        return ["编译失败"]
    # "Running tests/cases.rs (...)" 之后到下一个 Running 之间是这一层的输出。
    sections = re.split(r"\n\s+Running ", text)
    for section in sections:
        for key, label in LAYERS:
            if section.startswith(key) and "test result: FAILED" in section:
                caught.append(label)
    return caught


def main():
    wanted = set(sys.argv[1:])
    rows = []
    for bug_id, desc, path, old, new in BUGS:
        if wanted and bug_id not in wanted:
            continue
        file = ROOT / path
        original = file.read_text()
        assert original.count(old) == 1, f"{bug_id}: pattern not found exactly once"
        file.write_text(original.replace(old, new))
        try:
            caught = run_tests()
        finally:
            file.write_text(original)
        rows.append((bug_id, desc, "、".join(caught) if caught else "没有抓住"))
        print(f"{bug_id}\t{desc}\t{rows[-1][2]}", flush=True)
    assert run_tests() == [], "tests fail after restoring the code"


if __name__ == "__main__":
    main()
