# expect: error "expected 'module', found 'def'"
# 规则 1：第一个模块漏写了 module 关键字。
  def x 0
  text I 0
end
