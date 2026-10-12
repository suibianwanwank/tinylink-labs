# expect: warning "symbol 'helper' is defined but never used"
# 规则 4：只是警告，退出码仍然是 0。
module a
  def helper 1
  def main 0
  use main
  text
    E 1000
    I 0
end
