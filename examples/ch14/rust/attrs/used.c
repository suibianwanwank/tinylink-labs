/* used.c：C 的 used 与 used+retain，对照 Rust 的 #[used] */
static const char a[4] __attribute__((used)) = "aaa";
static const char b[4] __attribute__((used, retain)) = "bbb";
