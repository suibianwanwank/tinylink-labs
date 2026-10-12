use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        const {
            assert!(
                cfg!(all(target_os = "linux", target_arch = "x86_64")),
                "native x86-64 Linux required"
            );
        }
        let path = std::env::temp_dir().join(format!(
            "tinylink-class8-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn object(&self, source: &str) -> Vec<u8> {
        let input = self.0.join("input.s");
        let output = self.0.join("input.o");
        fs::write(&input, source).unwrap();
        let p = Command::new("as")
            .arg(&input)
            .arg("-o")
            .arg(&output)
            .output()
            .unwrap();
        assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
        fs::read(output).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn execute(path: &std::path::Path) -> i32 {
    // A concurrent fixture writer can briefly leave its writable descriptor in
    // a forked child before exec closes it. Retry only Linux ETXTBSY at spawn;
    // execution failures and all other spawn errors remain fatal.
    let spawn_deadline = Instant::now() + Duration::from_secs(3);
    let mut child = loop {
        match Command::new(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => break child,
            Err(e) if e.raw_os_error() == Some(26) && Instant::now() < spawn_deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => panic!("cannot execute fixture: {e}"),
        }
    };
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status.code().expect("generated program died from a signal");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("generated program timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn run_image(scratch: &Scratch, image: &[u8]) -> i32 {
    use std::os::unix::fs::PermissionsExt;
    let p = scratch.0.join("program");
    fs::write(&p, image).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    execute(&p)
}
use class8::archive::Input;
fn archive(scratch: &Scratch, entries: &[(&str, &str)]) -> Vec<u8> {
    let mut paths = Vec::new();
    for (name, source) in entries {
        let b = scratch.object(source);
        let p = scratch.0.join(name);
        fs::write(&p, b).unwrap();
        paths.push(p);
    }
    let out = scratch.0.join("library.a");
    let _ = fs::remove_file(&out);
    let p = Command::new("ar")
        .arg("rcs")
        .arg(&out)
        .args(paths)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    fs::read(out).unwrap()
}
fn kept(map: &str, name: &str) -> bool {
    map.lines()
        .any(|line| line.starts_with("KEEP ") && line.ends_with(&format!(" {name}")))
}
fn dropped(map: &str, name: &str) -> bool {
    map.lines()
        .any(|line| line.starts_with("DROP ") && line.ends_with(&format!(" {name}")))
}
const EXIT: &str = ".section .text.start,\"ax\",@progbits\n.globl _start\n_start: mov $42,%edi; mov $60,%eax; syscall\n";
#[test]
fn transitive_code_data_bss_and_dead_cycle() {
    let s = Scratch::new();
    let start=s.object(".section .text.start,\"ax\",@progbits\n.globl _start\n_start: call answer; mov %eax,%edi; mov $60,%eax; syscall\n");
    let worker=s.object(concat!(
        ".section .text.answer,\"ax\",@progbits\n.globl answer\nanswer: mov pointer(%rip),%rcx; movl $42,(%rcx); mov (%rcx),%eax; ret\n",
        ".section .data.pointer,\"aw\",@progbits\npointer: .quad storage\n",
        ".section .bss.storage,\"aw\",@nobits\nstorage: .skip 8192\n",
        ".section .text.dead1,\"ax\",@progbits\ndead1: call dead2; call missing; ret\n",
        ".section .text.dead2,\"ax\",@progbits\ndead2: call dead1; ret\n",
        ".section .rodata.unused,\"a\",@progbits\n.skip 16384\n",
    ));
    let inputs = [Input::Object(&start), Input::Object(&worker)];
    let prepared = class8::gc::prepare(&inputs, b"_start", &[]).unwrap();
    assert!(prepared.map_rows().iter().all(|row| row.address.is_none()));
    assert!(prepared
        .contributions()
        .iter()
        .all(
            |c| prepared.live().sections.contains(&class8::gc::SectionId {
                file: c.file,
                section: c.section
            })
        ));
    assert!(prepared
        .relocations()
        .iter()
        .all(|(id, _)| prepared.live().sections.contains(id)));
    assert!(prepared.map_rows().len() > prepared.contributions().len());
    let linked = class8::gc::link(&inputs, b"_start", &[]).unwrap();
    for name in [
        ".text.start",
        ".text.answer",
        ".data.pointer",
        ".bss.storage",
    ] {
        assert!(kept(&linked.map, name), "{}", linked.map);
    }
    for name in [".text.dead1", ".text.dead2", ".rodata.unused"] {
        assert!(dropped(&linked.map, name), "{}", linked.map);
    }
    assert_eq!(run_image(&s, &linked.bytes), 42);
    assert_eq!(class8::gc::link(&inputs, b"_start", &[]).unwrap(), linked);
    // Compare observable execution with a system linker for the same inputs.
    fs::write(s.0.join("start.o"), start).unwrap();
    fs::write(s.0.join("worker.o"), worker).unwrap();
    let oracle = s.0.join("oracle");
    let p = Command::new("ld")
        .args(["--gc-sections", "-e", "_start", "-o"])
        .arg(&oracle)
        .arg(s.0.join("start.o"))
        .arg(s.0.join("worker.o"))
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert_eq!(execute(&oracle), 42);
}
#[test]
fn explicit_roots_expose_dead_undefined() {
    let s = Scratch::new();
    let object=s.object(&format!("{EXIT}.section .text.optional,\"ax\",@progbits\n.globl optional\noptional: call missing; ret\n"));
    let inputs = [Input::Object(&object)];
    assert_eq!(
        run_image(
            &s,
            &class8::gc::link(&inputs, b"_start", &[]).unwrap().bytes
        ),
        42
    );
    assert_eq!(
        class8::gc::link(&inputs, b"_start", &[b"optional"]),
        Err(class8::gc::Error::Undefined(b"missing".to_vec()))
    );
    assert_eq!(
        class8::gc::link(&inputs, b"_start", &[b"absent"]),
        Err(class8::gc::Error::InvalidKeep(b"absent".to_vec()))
    );
}
#[test]
fn common_and_weak_storage_liveness() {
    let s = Scratch::new();
    let object=s.object(concat!(
        ".comm used,8192,64\n.comm unused,1048576,64\n.weak hook\n",
        ".section .text.start,\"ax\",@progbits\n.globl _start\n",
        "_start: movabs $hook,%rax; test %rax,%rax; jne bad; cmpl $0,used(%rip); jne bad; movl $42,used(%rip); mov used(%rip),%edi; jmp done\n",
        "bad: mov $9,%edi\ndone: mov $60,%eax; syscall\n",
    ));
    let linked = class8::gc::link(&[Input::Object(&object)], b"_start", &[]).unwrap();
    assert!(linked
        .map
        .lines()
        .any(|l| l.starts_with("COMMON ") && l.ends_with(" used")));
    assert!(!linked.map.contains("unused"));
    assert_eq!(run_image(&s, &linked.bytes), 42);
}
#[test]
fn selected_definition_controls_edges() {
    let s = Scratch::new();
    let weak=s.object(concat!(
        ".section .text.start,\"ax\",@progbits\n.globl _start\n_start: call answer; mov %eax,%edi; mov $60,%eax; syscall\n",
        ".section .text.weak,\"ax\",@progbits\n.weak answer\nanswer: call missing; ret\n",
    ));
    let strong = s.object(
        ".section .text.strong,\"ax\",@progbits\n.globl answer\nanswer: mov $42,%eax; ret\n",
    );
    let linked = class8::gc::link(
        &[Input::Object(&weak), Input::Object(&strong)],
        b"_start",
        &[],
    )
    .unwrap();
    assert!(dropped(&linked.map, ".text.weak"));
    assert!(kept(&linked.map, ".text.strong"));
    assert_eq!(run_image(&s, &linked.bytes), 42);
    // Global resolution precedes GC; unused duplicate definitions remain errors.
    let duplicate =
        s.object(".section .text.duplicate,\"ax\",@progbits\n.globl answer\nanswer: ret\n");
    assert!(matches!(
        class8::gc::link(
            &[
                Input::Object(&weak),
                Input::Object(&strong),
                Input::Object(&duplicate)
            ],
            b"_start",
            &[]
        ),
        Err(class8::gc::Error::Archives(
            class8::archive::Error::Resolution(class8::resolve::Error::Duplicate { .. })
        ))
    ));
}
#[test]
fn archive_selection_precedes_section_gc() {
    let s = Scratch::new();
    let start = s.object(&format!(
        "{EXIT}.section .text.dead,\"ax\",@progbits\ndead: call helper; ret\n"
    ));
    let ar = archive(
        &s,
        &[
            (
                "helper.o",
                ".section .text.helper,\"ax\",@progbits\n.globl helper\nhelper: ret\n",
            ),
            (
                "unused.o",
                ".section .text.unselected,\"ax\",@progbits\n.globl unused\nunused: ret\n",
            ),
        ],
    );
    let linked = class8::gc::link(
        &[Input::Object(&start), Input::Archive(&ar)],
        b"_start",
        &[],
    )
    .unwrap();
    assert!(dropped(&linked.map, ".text.helper"));
    assert!(!linked.map.contains(".text.unselected"));
    assert_eq!(run_image(&s, &linked.bytes), 42);
}
#[test]
fn dead_relocations_are_structural_but_not_applied() {
    let s = Scratch::new();
    let object = s.object(&format!(
        "{EXIT}.section .text.dead,\"ax\",@progbits\n.globl dead\ndead: call _start; ret\n"
    ));
    let sections = class8::sections::sections(&object).unwrap();
    let rela = sections
        .iter()
        .find(|(_, name)| *name == b".rela.text.dead")
        .unwrap()
        .0
        .offset as usize;
    let mut bad_kind = object.clone();
    bad_kind[rela + 8..rela + 12].copy_from_slice(&999u32.to_le_bytes());
    assert!(class8::gc::link(&[Input::Object(&bad_kind)], b"_start", &[]).is_ok());
    assert!(matches!(
        class8::gc::link(&[Input::Object(&bad_kind)], b"_start", &[b"dead"]),
        Err(class8::gc::Error::Output(
            class8::layout::Error::Relocation(_)
        ))
    ));
    let mut bad_offset = object.clone();
    bad_offset[rela..rela + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(class8::gc::link(&[Input::Object(&bad_offset)], b"_start", &[]).is_ok());
    assert!(class8::gc::link(&[Input::Object(&bad_offset)], b"_start", &[b"dead"]).is_err());
    let mut bad_index = object;
    bad_index[rela + 12..rela + 16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        class8::gc::link(&[Input::Object(&bad_index)], b"_start", &[]),
        Err(class8::gc::Error::Output(class8::layout::Error::Symbols(_)))
    ));
}
#[test]
fn none_and_metadata_relocations_do_not_retain_storage() {
    let s = Scratch::new();
    let object=s.object(&format!("{EXIT}.section .text.dead,\"ax\",@progbits\n.globl dead\ndead: ret\n.section .debug_info,\"\",@progbits\n.quad dead\n"));
    let linked = class8::gc::link(&[Input::Object(&object)], b"_start", &[]).unwrap();
    assert!(dropped(&linked.map, ".text.dead"));
    assert_eq!(run_image(&s, &linked.bytes), 42);
    let mut none = s.object(
        ".text\n.globl _start\n_start: mov $42,%edi; mov $60,%eax; syscall; .quad missing\n",
    );
    let rela = class8::sections::sections(&none)
        .unwrap()
        .iter()
        .find(|(_, n)| *n == b".rela.text")
        .unwrap()
        .0
        .offset as usize;
    none[rela + 8..rela + 12].copy_from_slice(&0u32.to_le_bytes());
    none[rela..rela + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        run_image(
            &s,
            &class8::gc::link(&[Input::Object(&none)], b"_start", &[])
                .unwrap()
                .bytes
        ),
        42
    );
}
#[test]
fn cli_writes_map_and_keeps_explicit_root() {
    let s = Scratch::new();
    let object = s.object(&format!(
        "{EXIT}.section .text.optional,\"ax\",@progbits\n.globl optional\noptional: ret\n"
    ));
    let input = s.0.join("input.o");
    fs::write(&input, object).unwrap();
    let output = s.0.join("output");
    let map = s.0.join("output.map");
    let p = Command::new(env!("CARGO_BIN_EXE_class8"))
        .arg("-o")
        .arg(&output)
        .arg("-Map")
        .arg(&map)
        .args(["--keep", "optional"])
        .arg(input)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert!(kept(&fs::read_to_string(map).unwrap(), ".text.optional"));
    assert_eq!(execute(&output), 42);
}

#[test]
fn roots_and_unsupported_sections_are_checked() {
    let s = Scratch::new();
    let object = s.object(&format!(
        "{EXIT}.globl absolute\n.set absolute,7\n.comm common,8,8\n.section .debug_info,\"\",@progbits\n.globl metadata\nmetadata: .byte 0\n.section .data.kept,\"aw\",@progbits\n.globl data_root\ndata_root: .quad 7\n"
    ));
    for name in [b"absolute".as_slice(), b"common", b"metadata"] {
        assert_eq!(
            class8::gc::link(&[Input::Object(&object)], b"_start", &[name]),
            Err(class8::gc::Error::InvalidKeep(name.to_vec()))
        );
        assert!(class8::gc::link(&[Input::Object(&object)], name, &[]).is_err());
    }
    let linked = class8::gc::link(&[Input::Object(&object)], b"_start", &[b"data_root"]).unwrap();
    assert!(kept(&linked.map, ".data.kept"));
    assert_eq!(run_image(&s, &linked.bytes), 42);
    let unsupported = s.object(&format!("{EXIT}.section .tbss,\"awT\",@nobits\n.skip 8\n"));
    assert_eq!(
        class8::gc::link(&[Input::Object(&unsupported)], b"_start", &[]),
        Err(class8::gc::Error::Output(
            class8::layout::Error::UnsupportedSection
        ))
    );
}
fn roles(section: &class8::sections::Section, name: &[u8]) -> Option<class8::gc::Role> {
    match (name, section.flags) {
        (b".annot", 2) => Some(class8::gc::Role::Annotation),
        (_, 0x32) => Some(class8::gc::Role::Split),
        (b".reject", _) => None,
        _ => class8::gc::storage_policy(section, name),
    }
}
#[test]
fn backend_roles_split_and_annotation() {
    use class8::gc::{Error, SectionId};
    let s = Scratch::new();
    let object = s.object(concat!(
        ".section .text.start,\"ax\",@progbits\n.globl _start\n_start: lea msg(%rip),%rax; ret\n",
        ".section .rodata.str,\"aMS\",@progbits,1\nmsg: .asciz \"live\"\n",
        ".section .rodata.unused,\"aMS\",@progbits,1\n.asciz \"dead\"\n",
        ".section .text.dead,\"ax\",@progbits\n.globl dead\ndead: call missing; ret\n",
        ".section .annot,\"a\",@progbits\n.globl described\ndescribed: .quad dead\n",
    ));
    let names = |id: SectionId| -> Vec<u8> {
        class8::sections::sections(&object).unwrap()[id.section as usize]
            .1
            .to_vec()
    };
    let inputs = [Input::Object(&object)];
    let prepared = class8::gc::prepare_with(&inputs, b"_start", &[], roles).unwrap();
    let split: Vec<_> = prepared.split().iter().map(|x| names(x.id)).collect();
    assert_eq!(split, [b".rodata.str".to_vec()]);
    assert_eq!(prepared.split()[0].bytes, b"live\0");
    assert_eq!(prepared.split()[0].flags, 0x32);
    assert_eq!(prepared.split()[0].entry_size, 1);
    let annotations: Vec<_> = prepared.annotations().iter().map(|x| names(x.id)).collect();
    assert_eq!(annotations, [b".annot".to_vec()]);
    assert_eq!(prepared.annotation_relocations().len(), 1);
    assert_eq!(
        prepared.annotation_relocations()[0].0,
        prepared.annotations()[0].id
    );
    let contributions: Vec<_> = prepared
        .contributions()
        .iter()
        .map(|c| {
            names(SectionId {
                file: c.file,
                section: c.section,
            })
        })
        .collect();
    assert_eq!(contributions, [b".text.start".to_vec()]);
    assert!(prepared
        .relocations()
        .iter()
        .all(|(id, _)| names(*id) == b".text.start"));
    assert!(!prepared
        .live()
        .sections
        .contains(&prepared.annotations()[0].id));
    let rows: Vec<_> = prepared.map_rows().iter().map(|r| r.name.clone()).collect();
    for name in [&b".rodata.unused"[..], b".annot", b".text.dead"] {
        assert!(rows.iter().any(|r| r == name));
    }
    assert!(matches!(
        class8::gc::prepare_with(&inputs, b"_start", &[b"described"], roles),
        Err(Error::InvalidKeep(name)) if name == b"described"
    ));
    let annotated = s.object(concat!(
        ".text\n.globl _start\n_start: lea described(%rip),%rax; ret\n",
        ".section .annot,\"a\",@progbits\ndescribed: .quad 0\n",
    ));
    assert!(matches!(
        class8::gc::prepare_with(&[Input::Object(&annotated)], b"_start", &[], roles),
        Err(Error::InvalidSymbol)
    ));
    let rejected =
        s.object(".text\n.globl _start\n_start: ret\n.section .reject,\"a\",@progbits\n.byte 0\n");
    assert!(matches!(
        class8::gc::prepare_with(&[Input::Object(&rejected)], b"_start", &[], roles),
        Err(Error::Output(class8::layout::Error::UnsupportedSection))
    ));
    assert!(matches!(
        class8::gc::prepare(&inputs, b"_start", &[]),
        Err(Error::Output(class8::layout::Error::UnsupportedSection))
    ));
}
