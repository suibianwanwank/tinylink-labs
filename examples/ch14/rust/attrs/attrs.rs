// attrs.rs：控制符号名、保留与放置
#![crate_type = "rlib"]

#[unsafe(no_mangle)]
pub extern "C" fn rs_area(w: i64, h: i64) -> i64 { w * h }

#[unsafe(no_mangle)]
pub fn rs_plain(x: i64) -> i64 { x + 1 }      // 不修饰，但仍是 Rust 调用约定

#[inline(never)]
pub extern "C" fn rs_c_mangled(x: i64) -> i64 { x + 2 }   // C 调用约定，名字照样修饰

fn helper(x: i64) -> i64 { x * 3 }            // 私有函数

#[used]
static KEEP_ME: [u8; 4] = *b"keep";            // 没人引用，也要留在目标文件里

static DROP_ME: [u8; 4] = *b"drop";            // 没人引用，编译器直接删掉

#[unsafe(link_section = ".note.mytag")]
#[used]
static TAG: [u8; 8] = *b"tag-v1\0\0";

#[inline(never)]
pub fn uses_helper(x: i64) -> i64 { helper(x) }
