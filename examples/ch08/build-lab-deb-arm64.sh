#!/bin/sh
# lab-deb-arm64 的构建脚本已挪到 examples/ch07/build-lab-deb-arm64.sh（第 7 章起用这个镜像）。
# 这里只保留转发，旧的引用路径照样能用。
exec sh "$(cd "$(dirname "$0")" && pwd)/../ch07/build-lab-deb-arm64.sh" "$@"
