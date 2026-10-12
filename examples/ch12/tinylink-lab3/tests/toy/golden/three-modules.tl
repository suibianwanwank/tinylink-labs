# expect: exit 0
# 三个模块互相引用：main 用到 print 和 limit，print 用到 buf。
# main 用到后面才定义的 print，也用到自己定义的 limit；print 用到更后面的 buf。
module main
  def limit 3
  use print limit
  text
    I 5000      # 立即数，不动
    E 1000      # 使用列表第 0 项 print
    E 2001      # 使用列表第 1 项 limit
    A 9007      # 绝对地址 7，不动
end

module print
  def print 1
  use buf
  text
    R 7000      # 本模块第 0 字
    E 6000      # buf
    R 8001      # 本模块第 1 字
end

module data
  def buf 0
  text
    I 0
    I 0
end
