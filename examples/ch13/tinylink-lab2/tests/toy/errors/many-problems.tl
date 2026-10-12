# expect: error "undefined symbol 'ghost'; using address 0"
# 一次输入里有多个问题：链接器不在第一个错误处停下，每处都按规则恢复并报告。
# 诊断按发现的先后排列：第一遍的在前，第二遍的在后，"定义了没用"最后按名字排序。
module a
  def b_thing 5
  def a_thing 0
  use ghost a_thing
  text
    E 1000
    R 2007
    A 3001
end
module b
  def a_thing 0
  use unused_here
  text I 0
end
