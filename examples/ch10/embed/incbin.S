        .section .rodata.blob, "a"
        .globl blob_start, blob_end
blob_start:
        .incbin "data/msg-v1.txt"
blob_end:
