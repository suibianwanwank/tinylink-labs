# expect: error "undefined symbol 'nope'; using address 0"
# 规则 3：使用列表里的 nope 没有任何模块定义，两处引用各报一次，地址都用 0。
module a
  use nope
  text
    E 1000
    I 1234
    E 2000
end
