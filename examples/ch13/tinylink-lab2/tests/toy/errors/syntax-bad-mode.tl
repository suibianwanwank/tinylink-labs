# expect: error "expected an address mode (I, A, R or E) or 'end', found 'X'"
# 规则 1：地址模式只有四种。
module a
  text
    I 1000
    X 2000
end
