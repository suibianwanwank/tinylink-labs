#!/bin/sh
# 第 13 章开头：结构体加字段之后，旧程序照常加载，结果却错了。
. "$(dirname "$0")/../env.sh"
stage break
run_deb sh -c 'cd break && sh steps.sh'
