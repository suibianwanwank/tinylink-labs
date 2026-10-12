# expect: exit 0
# 中间的模块长度为 0：它不占内存，后一个模块的基址和它的基址相同。
module a
  text I 1 I 2
end
module nothing
  text
end
module b
  def here 0
  use here
  text E 1000
end
