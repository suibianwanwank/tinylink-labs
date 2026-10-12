# expect: error "expected a symbol name, found 'end'"
# 规则 1：关键字不能当符号名。
module a
  def end 0
  text I 0
end
