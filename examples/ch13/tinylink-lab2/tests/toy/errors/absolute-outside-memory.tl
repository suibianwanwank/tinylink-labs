# expect: error "absolute address 500 is outside memory of 500 words; using address 0"
# 规则 8：内存只有 0 到 499，A 字的操作数 500 越界，操作码保留，地址换成 0。
module a
  text
    A 7499
    A 7500
end
