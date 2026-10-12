# tinylink

《链接器从零到精通》系列配套的链接器，从零开始用 Rust 写，每个实验一个 tag。

| tag | 内容 |
|---|---|
| `lab0` | 仓库骨架、数据驱动的测试框架、容器运行脚本 |
| `lab1` | M0：两遍玩具链接器（文本目标格式，带错误恢复规则） |
| `lab2` | M1：手写 ELF64 解析，`readelf`、`nm`；M2：把单个无重定位的 `.o` 写成可执行文件 |
| `lab3` | M4 前半：多个目标文件的全局符号表，强、弱、COMMON、未定义、重复定义，`resolve` 子命令 |

## 构建与测试

```console
$ cargo build
$ cargo test
$ cargo clippy --all-targets -- -D warnings
```

提交前这三条都要通过；clippy 用 `-D warnings` 把所有警告当错误。只用标准库，没有第三方依赖。

## 测试用例的写法

用例放在 `tests/<套件>/golden/` 和 `tests/<套件>/errors/` 下，每个用例是一个 `.tl` 输入文件，加上可选的 `.out`（期望 stdout）和 `.err`（期望 stderr）：

- 输入文件第一行声明预期结果：`# expect: exit N`、`# expect: error "文本"` 或 `# expect: warning "文本"`。
- `.out` 的比较忽略空白差异，规则同 `diff -b -B -E`。
- `.err` 逐字节比较。
- 缺少 `.out` 表示 stdout 必须为空，缺少 `.err` 表示 stderr 必须为空。
- `golden/` 下必须有 `.out`，`errors/` 下必须有 `.err`。
- `TINYLINK_CASE` 过滤后一个用例都不剩，算失败。

```console
$ TINYLINK_CASE=dup cargo test --test cases -- --nocapture   # 只跑名字里含 dup 的用例
$ TINYLINK_BLESS=1 cargo test --test cases                   # 用实际输出生成或覆盖期望文件，之后逐个检查差异
```

## 在 x86-64 Linux 容器里运行

本机是 Apple Silicon 的 macOS，链接出来的 x86-64 Linux 程序放进 colima 的容器运行：

```console
$ scripts/run-linux.sh path/to/program arg1 arg2
$ LINKER_LAB_PLATFORM=linux/arm64 scripts/run-linux.sh path/to/aarch64-program   # 原生 arm64 内核
$ LINKER_LAB_PROFILE=x86 scripts/run-linux.sh path/to/program                       # 真实 x86-64 内核
$ scripts/smoke-test.sh      # 用 musl 静态编译三个 C 程序：正常退出得 42，段错误得 139，死循环超时得 124
```

需要 colima（docker 运行时）、`alpine:3.20` 镜像，冒烟测试还需要 `x86_64-linux-musl-gcc`。程序会先复制到 `~/.cache/linker-lab/` 下再挂载，因为 colima 默认只共享 home 目录，挂载 `/tmp` 下的目录时容器里看到的是一个空目录。`docker run` 带了 `--init`：在这套环境里，崩溃的程序如果是容器的 1 号进程，容器会挂住不退出。脚本另有超时（`LINKER_LAB_TIMEOUT`，默认 60 秒），超时后杀掉容器并返回 124。脚本自己出错（参数、环境变量、平台与 `e_machine` 不符）时返回 125，和被测程序的退出码区分开。

默认的 x86-64 容器由 QEMU 用户态模拟执行，不经过 Linux 内核的 ELF 加载器。要在真实的 x86-64 内核上运行，先建一台 x86-64 虚拟机（Apple Silicon 上由 QEMU 整机模拟，较慢），再用 `LINKER_LAB_PROFILE=x86`：

```console
$ brew install lima-additional-guestagents
$ colima start -p x86 --arch x86_64 --cpu 2 --memory 2
$ docker context use colima        # colima start 会切换 docker 的当前 context，切回默认的那台
$ LINKER_LAB_PROFILE=x86 scripts/smoke-test.sh
```

## M0：玩具链接器

```console
$ cargo run -q -- toy tests/toy/golden/three-modules.tl
```

### 机器与格式

玩具机器字寻址，内存 500 个字，每个字是不超过四位的十进制数：千位是操作码，后三位是操作数。

输入由若干模块组成，记号之间用任意空白分隔，`#` 到行尾是注释：

```text
module main          # 模块名
  def limit 3        # 定义：符号 limit 在本模块偏移 3 处
  use print limit    # 使用列表：E 字的操作数是这里的下标，从 0 开始
  text               # 程序正文，每个字是"地址模式 值"
    I 5000           # I：立即数，不改
    E 1000           # E：操作数 0 是使用列表第 0 项 print，换成 print 的地址
    E 2001           # 操作数 1 是 limit
    A 9007           # A：绝对地址，不改，但必须小于 500
end                  # R（例子里没有）：相对本模块起点，加上模块基址
```

`module`、`def`、`use`、`text`、`end` 是关键字，不能用作符号名。

### 两遍

1. 第一遍（`src/toy/pass1.rs`）：按顺序给模块分配基址（下一个模块的基址 = 上一个的基址 + 长度），算出每个符号的绝对地址，建立符号表。
2. 第二遍（`src/toy/pass2.rs`）：用基址和符号表改写每个字，输出内存映像。

stdout 依次打印模块表、符号表（按名字排序）、内存映像。诊断写到 stderr，格式为 `文件:行:列: error|warning: 说明`。

### 错误规则

规则借鉴 NYU 操作系统课 Lab 1 的错误恢复规则（Hubertus Franke 版规格书，前身是 Allan Gottlieb 的作业），文本格式、报错文本和部分处理方式是本项目自己定的。每条规则在 `tests/toy/errors/` 下至少有一个用例。

| # | 情形 | 级别 | 处理 | 对应 NYU 规则 |
|---|---|---|---|---|
| 1 | 语法错误（缺记号、多记号、非法记号、数字溢出、非法 UTF-8），带行号和列号 | 错误，致命 | 立即停止，不输出 | 1 |
| 2 | 符号重复定义 | 错误 | 保留第一次的定义 | 2 |
| 3 | E 字引用了未定义的符号 | 错误 | 地址用 0 | 3 |
| 4 | 符号定义了但没有 E 字引用 | 警告 | | 4 |
| 5 | 定义的偏移超出本模块长度 | 错误 | 按偏移 0 处理；模块长度为 0 时定义作废，不进符号表 | 5（NYU 为警告） |
| 6 | E 字的下标超出使用列表 | 错误 | 按立即数处理 | 6 |
| 7 | 使用列表里的符号在本模块没有 E 字引用 | 警告 | | 7 |
| 8 | A 字的地址超出内存 | 错误 | 地址用 0 | 8 |
| 9 | R 字的地址超出本模块长度 | 错误 | 按偏移 0 处理，仍加基址 | 9 |
| 10 | 字超过四位 | 错误 | 换成 9999 | 10、11 合并 |
| 11 | 程序总长度超出内存 | 错误，致命 | 停止，不输出 | NYU 作为语法错误 `TOO_MANY_INSTR` |

有错误时退出码为 1；只有警告时为 0。非致命错误仍然打印完整的输出，方便看清每条规则恢复成了什么。stdout 的读端提前关闭（比如 `| head -1`）时静默停止输出，不算错误；其他写 stdout 的错误退出码为 2。

## M1：读 ELF

```console
$ tinylink readelf -h|-l|-S|-s|-r file.o   # 输出格式同 GNU readelf -W
$ tinylink nm file.o                       # 输出格式同 LC_ALL=C GNU nm
```

只支持 64 位小端 ELF，只用标准库解析（`src/elf/`）。输入被当作不可信数据：任何越界的偏移、大小、下标都返回带位置的错误，不会 panic。

- `tests/elf/fixtures/`：目标文件、生成它们的源码和脚本 `gen.sh`，以及 GNU binutils 2.45 的参考输出；`*.elf` 是 ld.lld 和 GNU ld 链接的两个可执行文件，用来对拍 `readelf -l`。`tests/elf_snapshots.rs` 逐字节对拍，有意的差异写在该文件开头。
- `tests/elf/errors/`：截断、`e_shoff` 越界、`sh_name` 越界、重定位符号编号越界、不是 ELF 的输入。
- `tests/elf_malformed.rs`：每个截断长度、各类越界、`sh_link` 类型不对、表项大小不对、扩展编号、没有节名字符串表，以及 5000 次固定种子的随机变异。

## M2：写出第一个可执行文件

```console
$ tinylink link -o exit42 tests/elf/fixtures/exit42.o
$ scripts/run-linux.sh ./exit42; echo $?                          # QEMU 用户态
42
$ LINKER_LAB_PROFILE=x86 scripts/run-linux.sh ./exit42; echo $?   # 真实 x86-64 内核
42
```

只接受一个没有重定位的目标文件，入口是全局符号 `_start`。输出是 ELF 头、一个 R+X 的 `PT_LOAD`（文件偏移 0 映射到 0x400000），以及各个只读可分配节的内容，没有节头表。有重定位、可写节、`.bss`、缺 `_start` 时报"尚不支持"，用例在 `tests/link/errors/`。

## M4 前半：符号解析

```console
$ tinylink resolve a.o b.o ...           # 打印每个全局名字的解析结果，错误一次报完
$ tinylink link -o out a.o b.o ...       # 多个没有重定位的输入；有重定位时报"尚不支持"
```

规则（`src/resolve/table.rs`）：强定义与强定义报重复定义（两个值相同的绝对符号除外）；强定义替换弱定义和 COMMON；两个弱定义保留先读到的；COMMON 替换弱定义；COMMON 之间取最大的尺寸和对齐；只有弱引用、没有定义的名字值为 0；有强引用、被可分配节里的重定位用到却没有定义的名字报未定义；局部符号不进全局表；可见性取最严的一档。`__ehdr_start`、`etext`、`edata`、`__bss_start`、`end` 等名字在没人定义时由链接器合成，值留到实验 5 的布局。没有 `_start` 时和 lld 一样警告，入口写 0。

- `tests/resolve/{golden,errors}/`：每个 `NAME.expect` 是一个用例，第二行 `# args:` 写完整命令行；输入由 `tests/resolve/gen.sh` 从 `tests/resolve/src/` 编译。
- `tests/resolve/linkers/`：`gen.sh` 用 ld.lld 21 和 GNU ld 2.45 链接同一组输入，由 `tests/resolve/verdict.py` 从映射文件和产物的 `readelf -s` 里摘出结论（每个名字的定义所在的文件、节、大小，COMMON 还有对齐）；lld 失败时它的 stderr 存为 `NAME.lld-stderr`。`tests/resolve_vs_linkers.rs` 要求 tinylink 的结论和它们一致，报错文字和 lld 逐字节相同；两个链接器不一致的 11 个用例、stderr 不比较的 3 个用例列在该文件开头，各有原因。
- `scripts/plant-bugs.py`：往 `src/resolve/` 里逐个埋 16 个 bug，运行 `cargo test`，列出每个 bug 被哪几层测试抓住，然后恢复代码。
