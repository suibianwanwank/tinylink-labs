# expect: error "word 12345 has more than 4 digits; using 9999"
# 规则 10：一个字最多四位。不论什么模式，都换成 9999，不再做别的改写。
module a
  use x
  text
    I 12345
    E 10000
end
module b
  def x 0
  text I 0
end
