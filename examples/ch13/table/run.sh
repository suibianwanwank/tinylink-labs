#!/bin/sh
# 第 13 章"ABI 是什么"：enum、内联函数、虚函数表三类不兼容改动。C++ 部分要 g++，用 lab-deb-arm64-abi。
. "$(dirname "$0")/../env.sh"
stage table
run_abi sh -c 'cd table && sh steps.sh'
