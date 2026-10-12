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
fn native_cross_object_resolution() {
    const {
        assert!(
            cfg!(all(target_os = "linux", target_arch = "x86_64")),
            "run this integration test on native x86-64 Linux"
        );
    }
    let path = std::env::temp_dir().join(format!(
        "tinylink-class3-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    let scratch = Scratch(path);
    let mut objects = Vec::new();
    for (n, assembly) in [
        ("caller", ".text\n.globl start\nstart: call answer; ret\n"),
        (
            "provider",
            ".text\n.globl answer\nanswer: mov $42, %eax; ret\n",
        ),
    ] {
        let input = scratch.0.join(format!("{n}.s"));
        let output = scratch.0.join(format!("{n}.o"));
        fs::write(&input, assembly).unwrap();
        let out = Command::new("as")
            .arg(&input)
            .arg("-o")
            .arg(&output)
            .output()
            .unwrap();
        assert!(out.status.success());
        objects.push(fs::read(output).unwrap());
    }
    let mut all_sections = Vec::new();
    let mut tables = Vec::new();
    for bytes in &objects {
        let sections: Vec<_> = class3::sections::sections(bytes)
            .unwrap()
            .into_iter()
            .map(|(s, _)| s)
            .collect();
        let table = sections.iter().position(|s| s.kind == 2).unwrap();
        tables.push(class3::symbols::symbols(bytes, &sections, table).unwrap());
        all_sections.push(sections);
    }
    let globals = class3::resolve::build_globals(&tables).unwrap();
    let reloc = all_sections[0].iter().position(|s| s.kind == 4).unwrap();
    let records = class3::symbols::relocations(&objects[0], &all_sections[0], reloc).unwrap();
    let r = records
        .iter()
        .find(|r| tables[0][r.symbol as usize].name == b"answer")
        .unwrap();
    let provider = tables[1].iter().position(|s| s.name == b"answer").unwrap();
    assert_eq!(
        class3::resolve::bind(
            &tables,
            &globals,
            class3::resolve::SymbolId {
                file: 0,
                symbol: r.symbol as usize
            }
        ),
        Ok(class3::resolve::Binding::Selected(
            class3::resolve::Selection::Definition(class3::resolve::SymbolId {
                file: 1,
                symbol: provider
            })
        ))
    );
}
