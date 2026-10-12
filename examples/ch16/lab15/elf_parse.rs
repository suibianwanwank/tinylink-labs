#![no_main]
//! 第 16 章练习四：fuzz 的入口是 ElfFile::parse。
//! 解析成功的输入再交给 nm 走一遍符号表，后面的代码才真正被执行到。
use libfuzzer_sys::fuzz_target;
use tinylink::elf::ElfFile;
use tinylink::nm::NmView;

fuzz_target!(|data: &[u8]| {
    if let Ok(file) = ElfFile::parse(data) {
        if let Ok(view) = NmView::new(&file) {
            let _ = view.to_string();
        }
    }
});
