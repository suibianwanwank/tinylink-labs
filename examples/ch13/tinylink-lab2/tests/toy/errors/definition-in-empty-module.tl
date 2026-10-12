# expect: error "symbol 'q' is defined in module 'empty', which has no words; ignoring the definition"
# 规则 5 的特例：empty 模块一个字都没有。如果照常按偏移 0 处理，q 会得到 empty 的基址 2，
# 那已经在所有模块之外了。所以这个定义作废、不进符号表，引用 q 的 E 字按未定义符号处理。
module a
  use q
  text E 1000 I 0
end
module empty
  def q 0
  text
end
