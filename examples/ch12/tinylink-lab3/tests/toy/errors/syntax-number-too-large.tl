# expect: error "number '4294967296' is too large"
# 规则 1：数字要放得进 32 位无符号整数（最大 4294967295）。
module a
  text I 4294967296
end
