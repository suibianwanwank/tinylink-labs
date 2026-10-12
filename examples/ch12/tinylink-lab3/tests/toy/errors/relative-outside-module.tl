# expect: error "relative address 3 is outside module 'b' of 3 words; using offset 0"
# 规则 9：模块 b 只有 0 到 2 三个字。地址按偏移 0 处理，仍要加上 b 的基址 1。
module a
  text I 0
end
module b
  text
    R 1002
    R 1003
    I 0
end
