# expect: error "symbol 'far' is defined at offset 2, but module 'b' has only 2 words; using offset 0"
# 规则 5：b 只有偏移 0、1 两个字，偏移 2 刚好越界。按偏移 0 处理，地址是 b 的基址 1。
module a
  use far
  text E 1000
end
module b
  def far 2
  text I 0 I 0
end
