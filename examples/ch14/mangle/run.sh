#!/bin/sh
# 名字修饰：看编译器生成的名字，再用三种 demangler 还原
. "$(dirname "$0")/../env.sh"; stage mangle
set -x
$GXX -O0 -c names.cpp
$NM names.o | grep -v ' U '
$NM names.o | grep -v ' U ' | awk '{print $3}' | c++filt
$NM names.o | grep -v ' U ' | awk '{print $3}' | llvm-cxxfilt
c++filt --version | head -1; llvm-cxxfilt --version | head -2
# 替换：同一个名字的前缀在后面复现
llvm-cxxfilt _ZN3geo4moveERNS_5PointERKS0_ _ZN1N1TIiiE2mfES0_IddE
# 模板名进替换字典：S_ 是 biggest，S0_ 是 T_，S1_ 不存在
for t in _Z7biggestIlET_S0_S_ _Z7biggestIlET_S0_S0_ _Z7biggestIlET_S0_S1_ _Z3addIiET_S0_S0_; do
  printf '%s  llvm-cxxfilt: %s  c++filt: %s\n' $t "$(llvm-cxxfilt $t)" "$(c++filt $t)"
done
# 格式对照：在 Linux 上生成 Mach-O 目标文件，仅检查符号，不执行
clang++ --target=arm64-apple-macos -c ms.cpp -o names_macho.o && llvm-nm names_macho.o | grep geo4move
llvm-cxxfilt __ZN3geo4moveERNS_5PointERKS0_; llvm-cxxfilt -n __ZN3geo4moveERNS_5PointERKS0_
# 两个函数模板的实例名字相同、返回类型不同：修饰名里带上返回类型才能区分
$GXX -O0 -c tget1.cpp && $GXX -O0 -c tget2.cpp
$NM tget1.o tget2.o | grep get
# MSVC 的修饰规则（clang 的 msvc 目标）
clang++ --target=x86_64-pc-windows-msvc -c ms.cpp -o ms.obj && llvm-nm ms.obj | grep -v ' U '
llvm-nm ms.obj | grep -v ' U ' | awk '{print $3}' | grep '^?' | llvm-undname
exit $FAIL
