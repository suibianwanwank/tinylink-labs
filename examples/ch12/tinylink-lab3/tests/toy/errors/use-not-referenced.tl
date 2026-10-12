# expect: warning "symbol 'y' at index 1 of the use list of module 'a' is not referred to by any E word"
# 规则 7：use 里列了 y，正文没有 E 字用它。只是警告。
module a
  def x 0
  def y 1
  use x y
  text
    E 1000
    I 0
end
module b
  use y
  text E 2000
end
