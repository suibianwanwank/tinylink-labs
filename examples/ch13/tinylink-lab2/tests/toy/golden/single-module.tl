# expect: exit 0
# 只有一个模块，没有符号：I 和 A 原样保留，R 加上基址 0 也不变。
module solo
  text
    I 1234
    A 2003
    R 3001
    R 4000
end
