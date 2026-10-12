#!/bin/sh
# 第 13 章"符号版本的实践"：同一个函数的两个实现，旧程序绑旧的，新程序绑新的。
. "$(dirname "$0")/../env.sh"
stage symver
run_deb sh -c 'cd symver && sh steps.sh'
