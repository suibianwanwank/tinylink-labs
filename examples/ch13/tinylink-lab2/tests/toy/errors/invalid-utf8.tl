# expect: error "input is not valid UTF-8"
# 规则 1：第 4 行末尾有一个字节 0xFF，它在 UTF-8 里不可能出现。位置指向这个字节。
module a
  text I 1�
end
