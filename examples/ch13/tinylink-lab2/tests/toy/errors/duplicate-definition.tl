# expect: error "symbol 'x' is already defined in module 'a' (line 4); keeping the first definition"
# 规则 2：x 在两个模块里都有定义，保留第一个（地址 1），第二个（地址 2）丢弃。
module a
  def x 1
  text I 0 I 0
end
module b
  def x 0
  use x
  text E 1000
end
