#!/usr/bin/env bash
# 在 colima 的容器里运行一个 Linux 程序，退出码原样传回。
#
# 用法：scripts/run-linux.sh <程序> [参数...]
#
# 环境变量：
#   LINKER_LAB_DIR      放临时文件的目录，默认 ~/.cache/linker-lab，必须在 $HOME 下
#   LINKER_LAB_TIMEOUT  最长运行秒数，默认 60；超时后杀掉容器，退出码 124
#   LINKER_LAB_PROFILE  用哪台 colima 虚拟机，默认 default（arm64 虚拟机）；
#                       设成 x86 时用 `colima start -p x86 --arch x86_64` 建的 x86-64 虚拟机，
#                       程序在真正的 x86-64 Linux 内核上运行（虚拟机本身由 QEMU 模拟整机，较慢）
#   LINKER_LAB_PLATFORM 容器平台，linux/amd64 或 linux/arm64，默认 linux/amd64。
#                       default profile 上，amd64 由 QEMU 用户态模拟执行，arm64 在虚拟机的
#                       arm64 内核上原生运行；x86 profile 上只能用 amd64，在 x86-64 内核上原生运行
#
# 脚本自己出错时（用法、配置、环境）退出码是 125，和 `docker run`、GNU timeout 的约定一样，
# 免得和被测程序的退出码混在一起；超时是 124。
#
# 程序会被复制到 $LINKER_LAB_DIR 下的一个临时目录，挂载为容器里的 /w，运行结束后删除。
# colima 默认只把 home 共享进虚拟机，/tmp 里的文件在容器里看不到。
#
# --init 让 docker 的 init 进程当 1 号进程，被测程序作为它的子进程运行。
# 在这套 colima 环境里，崩溃的程序如果自己是 1 号进程，容器会挂住不退出。

set -euo pipefail

fail() {
    echo "run-linux: $*" >&2
    exit 125
}

if [[ $# -lt 1 ]]; then
    echo "usage: $0 <program> [args...]" >&2
    exit 125
fi

prog=$1
shift

[[ -f $prog ]] || fail "no such file: $prog"

lab=${LINKER_LAB_DIR:-$HOME/.cache/linker-lab}
timeout=${LINKER_LAB_TIMEOUT:-60}
platform=${LINKER_LAB_PLATFORM:-linux/amd64}
profile=${LINKER_LAB_PROFILE:-default}

# 下面所有检查都要在启动 docker、设置 EXIT trap 之前做完。macOS 自带的 bash 3.2 有个坑：
# 设了 EXIT trap 之后，`set -u` 遇到未定义变量（比如 timeout=abc 时算术表达式里的 abc）
# 退出时的退出码是 0。那时 docker 已经在后台跑着、失去了超时看守，trap 删掉挂载目录后，
# docker 还会在原处重建一个空目录。
[[ $timeout =~ ^[0-9]+$ ]] || fail "LINKER_LAB_TIMEOUT must be a number of seconds, got '$timeout'"

# colima 为每个 profile 建一个 docker context：default 对应 colima，其他对应 colima-<profile>。
case $profile in
    default) context=colima ;;
    x86) context=colima-x86 ;;
    *) fail "unknown LINKER_LAB_PROFILE '$profile' (expected default or x86)" ;;
esac
case $platform in
    linux/amd64) machine=62 ;;   # EM_X86_64
    linux/arm64) machine=183 ;;  # EM_AARCH64
    *) fail "unknown LINKER_LAB_PLATFORM '$platform' (expected linux/amd64 or linux/arm64)" ;;
esac
if [[ $profile == x86 && $platform != linux/amd64 ]]; then
    fail "the x86 profile can only run linux/amd64 programs"
fi
# 程序是 ELF 文件时，先核对 e_machine（偏移 18 的两个字节，小端）。平台不对时，
# 有的组合会悄悄交给 QEMU 模拟，有的会报 exec format error，都不如在这里说清楚。
if [[ $(head -c 4 "$prog" | od -An -c | tr -d ' ') == '177ELF' ]]; then
    found=$(od -An -tu2 -j18 -N2 "$prog" | tr -d ' ')
    [[ $found == "$machine" ]] ||
        fail "$prog has e_machine $found, but LINKER_LAB_PLATFORM=$platform runs e_machine $machine"
fi
if ! docker --context "$context" info >/dev/null 2>&1; then
    if [[ $profile == x86 ]]; then
        fail "docker context '$context' is not running; try: colima start -p x86 --arch x86_64"
    fi
    fail "docker context '$context' is not running; try: colima start"
fi

not_under_home() {
    fail "$lab is not under \$HOME; colima cannot mount it"
}

# 先按字面检查，避免在 home 以外建目录；建好后再按真实路径检查一次，防止符号链接指到别处。
case $lab in
    "$HOME"/*) ;;
    *) not_under_home ;;
esac
mkdir -p "$lab"
real_lab=$(cd "$lab" && pwd -P)
real_home=$(cd "$HOME" && pwd -P)
case $real_lab in
    "$real_home"/*) ;;
    *) not_under_home ;;
esac

work=$(mktemp -d "$real_lab/run.XXXXXX")
name="linker-lab-${work##*.}"
trap 'rm -rf "$work"' EXIT

cp "$prog" "$work/prog"
chmod +x "$work/prog"

# 非交互 shell 里，放到后台的命令默认从 /dev/null 读标准输入，先把真正的 stdin 存到 fd 3。
exec 3<&0
docker --context "$context" run --rm --init -i --name "$name" --platform "$platform" \
    -v "$work:/w" -w /w alpine:3.20 /w/prog "$@" <&3 &
pid=$!

# 每 0.2 秒看一次容器还在不在，超时就杀掉。
deadline=$((SECONDS + timeout))
while kill -0 "$pid" 2>/dev/null; do
    if ((SECONDS >= deadline)); then
        echo "run-linux: timed out after ${timeout}s; killing container $name" >&2
        docker --context "$context" kill "$name" >/dev/null 2>&1 || true
        wait "$pid" 2>/dev/null || true
        exit 124
    fi
    sleep 0.2
done

status=0
wait "$pid" || status=$?
exit "$status"
