#!/bin/bash
# Linux 主机运行 RISC-V 目标；超时表示停止观察，不视作程序主动退出。
set -eu
set +e
timeout --signal=TERM --kill-after=1s 2s qemu-system-riscv64 -machine virt -bios none -m 128M -nographic -kernel "$@" </dev/null
rc=$?
set -e
[ "$rc" -eq 0 ] || [ "$rc" -eq 124 ]
