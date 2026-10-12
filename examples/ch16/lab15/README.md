# 实验 15：Linux 原生差分测试

差分驱动的唯一实现位于最终整合源码 `tinylink-complete/scripts/difftest.py`。在原生 x86-64 Linux 上进入整合源码根目录，执行：

```sh
cargo build --release
python3 scripts/test-difftest.py
python3 scripts/difftest.py . --first 1 --count 20
```

完整验收与参数说明见实验 15 正文及整合源码的 README。本目录不再保留单独的运行驱动，以免示例与实际验收逻辑分叉。
