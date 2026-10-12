# The World of Linkers — companion sources / 配套源码

These are the source files, scripts, and expected-output fixtures referenced by
Theory 00–16. They are separate from the `class0`–`class15` implementation labs.
This repository is private; a GitHub account with repository access is required.

本目录提供原理篇 00–16 引用的源码、脚本和预期输出。`class0`–`class15`
仍是逐步实现链接器的实验项目，不需要用这里的辅助工具替代自己的实现。
仓库保持 private，获取代码需要 GitHub 仓库访问权限。

## Where commands run / 命令的位置

Clone the repository on native x86-64 Linux. Paths beginning with `examples/`
in the articles are relative to the cloned repository root. For example:

```sh
git clone https://github.com/suibianwanwank/tinylink-labs.git
cd tinylink-labs
python3 examples/ch00/run-linux.py
```

正文中 `examples/` 开头的路径相对于克隆后的仓库根目录。它们不是作者机器上
预先存在的路径。脚本生成的产物应在脚本打印的目录中查看，不要照抄示例输出
中的地址、进程号或临时目录名。若正文使用 `W`，将它设为本次脚本输出的产物
目录；若使用 `SOURCE_ROOT`，在仓库根目录执行 `export SOURCE_ROOT="$PWD"`。

## Environment / 环境

Read the tool and version requirements in the corresponding article before
running a chapter. The scripts do not install toolchains. Depending on the
chapter, they require GCC, Clang/LLD/LLVM, GNU binutils, musl tools, Rust, Python,
GDB, make, and/or QEMU with RISC-V tools. Some scripts download third-party source
or package archives. Version-specific expected output can differ on another
toolchain. The sources are provided here; this publication does not claim a new
full execution of every chapter on every Linux distribution.

运行前请查看对应文章的依赖与版本。脚本不会自动安装工具链；部分章节会下载
第三方源码或软件包。版本不同可能导致预期输出不同。xv6/RISC-V 和其他文件
格式的对照遵循对应文章的说明。此处提供源码，不代表所有脚本已经在所有
Linux 发行版上重新验证。
