# expect: error "external index 1 is out of range for the use list of module 'a' (length 1); treating the word as immediate"
# 规则 6：使用列表只有一项（下标 0），E 3001 要下标 1，刚好越界，按立即数原样保留。
module a
  def x 0
  use x
  text
    E 1000
    E 3001
end
