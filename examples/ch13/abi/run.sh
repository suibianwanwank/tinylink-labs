#!/bin/sh
# 第 13 章"ABI 检查工具"：abidiff 与 abi-compliance-checker。
. "$(dirname "$0")/../env.sh"
stage abi
run_abi sh -c 'cd abi && sh steps.sh'
