#!/bin/sh
# Linux x86-64 原生 glibc 版本对照，旧 RPM 仅解包到实验目录。
set -e
. "$(dirname "$0")/../env.sh"
stage glibc
# CentOS 7 aarch64 的 glibc 开发包，从 vault.centos.org 下载后用 bsdtar 解到 $W/sysroot-el7
B=https://vault.centos.org/7.9.2009/os/x86_64/Packages
if [ ! -f "$W/sysroot-el7/usr/lib64/Scrt1.o" ]; then
  mkdir -p "$W/rpms" "$W/sysroot-el7" && find "$W/sysroot-el7" -mindepth 1 -delete
  for p in glibc-2.17-317.el7.x86_64.rpm glibc-devel-2.17-317.el7.x86_64.rpm \
           glibc-headers-2.17-317.el7.x86_64.rpm kernel-headers-3.10.0-1160.el7.x86_64.rpm; do
    if [ ! -f "$W/rpms/$p" ]; then
      curl -fsSL -o "$W/rpms/$p" "$B/$p" || { echo "ERROR: 下载 $B/$p 失败" >&2; rm -f "$W/rpms/$p"; exit 1; }
    fi
    bsdtar -tf "$W/rpms/$p" >/dev/null || { echo "ERROR: $p 不是完整的 RPM，已删除，请重跑" >&2; rm -f "$W/rpms/$p"; exit 1; }
    bsdtar -xf "$W/rpms/$p" -C "$W/sysroot-el7"
  done
fi
run_deb sh -c 'cd glibc && sh steps.sh'
# 用解包得到的旧加载器直接启动，同一个 Linux 内核，无需容器或模拟器。
(cd "$W/glibc"; for p in th th_pin fm_old th_el7; do
 echo "== glibc 2.17: $p"
 "$W/sysroot-el7/lib64/ld-linux-x86-64.so.2" --library-path "$W/sysroot-el7/lib64:$W/sysroot-el7/usr/lib64" "./$p" || echo "exit=$?"
done)
run_abi sh "$W/glibc/x86.sh"
