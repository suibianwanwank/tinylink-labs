# expect: exit 0
# 每条规则的边界值都还合法：A 499、R 指向模块最后一个字、9999、E 用最后一个下标。
module a
  def x 4
  use y x
  text
    A 1499
    R 2004
    E 3001
    E 4000
    I 9999
end
module b
  def y 0
  text
    R 0000
end
