# expect: exit 0
# 同一个符号在使用列表里出现两次也合法，两个下标都指向它。
module a
  def x 0
  use x x
  text
    E 1000
    E 1001
end
