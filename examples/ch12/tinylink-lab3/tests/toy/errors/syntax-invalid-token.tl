# expect: error "invalid token '10x4'"
# 规则 1：记号要么全是数字，要么是名字。下一行开头是一个 Tab，Tab 算一列。
module a
  text
	R 10x4
end
