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
fn native_assembler_sections() {
    const {
        assert!(
            cfg!(all(target_os = "linux", target_arch = "x86_64")),
            "run this integration test on native x86-64 Linux"
        );
    }
    let path = std::env::temp_dir().join(format!(
        "tinylink-class1-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    let scratch = Scratch(path);
    let input = scratch.0.join("input.s");
    let output = scratch.0.join("input.o");
    fs::write(
        &input,
        ".text\n.globl answer\nanswer: mov $42, %eax; ret\n.bss\n.space 32\n",
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
    let sections = class1::sections::sections(&bytes).unwrap();
    let (text, _) = sections.iter().find(|(_, name)| *name == b".text").unwrap();
    assert_eq!(
        class1::sections::payload(&bytes, text).unwrap(),
        &[0xb8, 42, 0, 0, 0, 0xc3]
    );
    let (bss, _) = sections.iter().find(|(_, name)| *name == b".bss").unwrap();
    assert_eq!(bss.kind, 8);
    assert_eq!(bss.size, 32);
    assert!(class1::sections::payload(&bytes, bss).unwrap().is_empty());
}
