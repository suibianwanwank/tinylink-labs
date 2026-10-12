# expect: error "expected an address mode (I, A, R or E) or 'end', found end of file"
# 规则 1：文件在模块中间结束。报告位置是最后一行的行尾。
module a
  text
    I 1000
