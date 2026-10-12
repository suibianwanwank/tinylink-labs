# anyc1.s：同样两份不同内容，但选择规则是 ANY（discard）
    .section .rdata,"dr",discard,aval
    .globl aval
aval:
    .long 2
