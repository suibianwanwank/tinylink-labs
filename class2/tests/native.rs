//! Run on native x86-64 Linux; binutils supplies the independent object producer.
use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn native_assembler_relocations() {
    const {
        assert!(
            cfg!(all(target_os = "linux", target_arch = "x86_64")),
            "run this integration test on native x86-64 Linux"
        );
    }
    let path = std::env::temp_dir().join(format!(
        "tinylink-class2-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    let scratch = Scratch(path);
    let input = scratch.0.join("input.s");
    let output = scratch.0.join("input.o");
    fs::write(
        &input,
        ".text\n.globl answer\nanswer: call external_function; ret\n.bss\n.space 32\n",
    )
    .unwrap();
    let result = Command::new("as")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "assembler: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = fs::read(&output).unwrap();
    let named = class2::sections::sections(&bytes).unwrap();
    let sections: Vec<_> = named.into_iter().map(|(s, _)| s).collect();
    let index = sections.iter().position(|s| s.kind == 4).unwrap();
    let records = class2::symbols::relocations(&bytes, &sections, index).unwrap();
    let syms = class2::symbols::symbols(&bytes, &sections, sections[index].link as usize).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].offset, 1);
    assert_eq!(records[0].kind, 4);
    assert_eq!(records[0].addend, -4);
    assert_eq!(syms[records[0].symbol as usize].name, b"external_function");
    assert_eq!(
        syms[records[0].symbol as usize].definition,
        class2::symbols::Definition::Undefined
    );
}
