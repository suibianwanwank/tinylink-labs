#!/bin/bash
# 用法：./build.sh 链接脚本 输出名
set -e
CF="--target=riscv64-unknown-elf -march=rv64gc -mabi=lp64d -mcmodel=medany -mno-relax -ffreestanding -fno-builtin -nostdlib -O1"
clang $CF -c entry.S -o entry.o
clang $CF -c main.c -o main.o
ld.lld -T "$1" -o "$2" entry.o main.o
