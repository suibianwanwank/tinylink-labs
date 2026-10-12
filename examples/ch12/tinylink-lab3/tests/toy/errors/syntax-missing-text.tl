# expect: error "expected 'def', 'use' or 'text', found 'R'"
# 规则 1：忘了写 text，直接开始写程序正文。
module a
  def x 0
  R 1000
end
