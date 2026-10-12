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
            "tinylink-class11-{}-{}",
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
use class11::archive::Input;

#[test]
fn lowering_preserves_layout_before_startup_serialization() {
    let scratch = Scratch::new();
    let object = scratch.object(
        r#"
        .text
        .globl _start
    _start:
        mov counter@gottpoff(%rip), %rax
        mov %fs:(%rax), %edi
        mov $60, %eax
        syscall
        .section .tdata,"awT",@progbits
        .p2align 2
        .globl counter
        .type counter,@tls_object
    counter:
        .long 42
        .size counter,4
        .data
        .p2align 3
        .globl saved_entry
    saved_entry:
        .quad _start
        .section .note.GNU-stack,"",@progbits
        "#,
    );
    let inputs = [Input::Object(&object)];
    let prepared =
        class11::gc::prepare_with(&inputs, b"_start", &[b"saved_entry"], class11::tls::policy)
            .expect("fixture has valid storage, TLS and references");
    let lowered = class11::tls::lower_prepared(&prepared).expect("PIE lowering succeeds");

    assert!(lowered.image.bytes[..64].iter().all(|b| *b == 0));
    let tls = lowered.tls.as_ref().expect("live initial-exec TLS");
    assert_eq!(tls.image, 42u32.to_le_bytes());
    assert_eq!((tls.memory_size, tls.thread_pointer), (4, 8));
    let got = &lowered.image.placements[&class11::tls::GOT];
    assert_eq!(got.size, 8);
    let at = got.file_offset.unwrap() as usize;
    assert_eq!(&lowered.image.bytes[at..at + 8], &(-8i64).to_le_bytes());

    assert_eq!(lowered.relative.len(), 1, "saved_entry needs load bias");
    let record = &lowered.relative[0];
    assert_eq!(record.addend as u64, lowered.entry_rva);
    assert_eq!(
        &lowered.image.bytes[record.offset as usize..record.offset as usize + 8],
        &[0; 8],
        "the startup stage supplies the address at load time"
    );
    let entry_symbol = prepared.tables()[0]
        .iter()
        .find(|symbol| symbol.name == b"_start")
        .unwrap();
    let class11::symbols::Definition::Section(section) = entry_symbol.definition else {
        panic!("fixture entry must be defined in text")
    };
    let entry_placement = &lowered.image.placements[&(0, section)];
    assert_eq!(
        lowered.entry_rva,
        entry_placement.address - class11::executable::BASE + entry_symbol.value
    );

    let output = lowered.emit().expect("serialize the retained layout");
    assert_eq!(&output.bytes[..4], b"\x7fELF");
    assert_eq!(
        u16::from_le_bytes(output.bytes[16..18].try_into().unwrap()),
        3
    );
    assert_eq!(
        u16::from_le_bytes(output.bytes[56..58].try_into().unwrap()),
        5
    );
    assert!(output.map.contains(".tls.image") && output.map.contains(".got"));
    for _ in 0..5 {
        assert_eq!(run_image(&scratch, &output.bytes), 42);
    }
}

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
/// Compile freestanding C without unwind tables (class9 PIE output has none).
fn compile(s: &Scratch, name: &str, source: &str, extra: &[&str]) -> Vec<u8> {
    let input = s.0.join(format!("{name}.c"));
    let output = s.0.join(format!("{name}.o"));
    fs::write(&input, source).unwrap();
    let p = Command::new("cc")
        .args([
            "-c",
            "-O1",
            "-ffreestanding",
            "-fno-tree-loop-distribute-patterns",
            "-ffunction-sections",
            "-fdata-sections",
            "-fno-stack-protector",
            "-fcf-protection=none",
        ])
        .args(["-fno-asynchronous-unwind-tables", "-fno-unwind-tables"])
        .args(extra)
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    fs::read(output).unwrap()
}
fn u16at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(b[at..at + 2].try_into().unwrap())
}
fn u32at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn u64at(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}
/// Program headers as (type, flags, offset, vaddr, filesz, align).
fn program_headers(elf: &[u8]) -> Vec<(u32, u32, u64, u64, u64, u64)> {
    let (offset, count) = (u64at(elf, 32) as usize, u16at(elf, 56) as usize);
    (0..count)
        .map(|i| {
            let h = offset + i * 56;
            (
                u32at(elf, h),
                u32at(elf, h + 4),
                u64at(elf, h + 8),
                u64at(elf, h + 16),
                u64at(elf, h + 32),
                u64at(elf, h + 48),
            )
        })
        .collect()
}
const START: &str = ".section .text.start,\"ax\",@progbits\n.globl _start\n_start: mov %rsp,%rdi; call check; mov %eax,%edi; mov $231,%eax; syscall\n";
const DEFINITIONS: &str = "__thread int remote = 11;\nint global = 7;\n";
const LOCALS: &str = r#"
__thread int counter = 5;
__thread long zeroed;
static __thread char local_buf[3] = {1, 2, 3};
extern __thread int remote;
extern int global;
__thread int *tls_pointer = &global;
__attribute__((noipa)) int *where(void) { return &counter; }
int check(void) {
    if (counter != 5 || zeroed != 0 || local_buf[2] != 3 || remote != 11 || *tls_pointer != 7)
        return 1;
    counter++; zeroed += 2; local_buf[1]++; remote++;
    if (counter != 6 || zeroed != 2 || local_buf[1] != 3 || remote != 12) return 2;
    if (where() != &counter) return 3;
    *where() = 40;
    if (counter + (int)zeroed != 42) return 4;
    long tp;
    __asm__ volatile("mov %%fs:0, %0" : "=r"(tp));
    if ((char *)&counter >= (char *)tp || (char *)&remote >= (char *)tp) return 5;
    return 42;
}
"#;
fn map_line<'m>(map: &'m str, name: &str) -> Option<&'m str> {
    map.lines()
        .find(|l| l.starts_with("SYNTHETIC ") && l.ends_with(&format!(" {name}")))
}
fn synthetic_size(map: &str, name: &str) -> Option<u64> {
    map_line(map, name).map(|l| l.split(' ').nth(2).unwrap().parse().unwrap())
}
#[test]
fn thread_locals_start_with_their_initial_values() {
    let s = Scratch::new();
    let start = s.object(START);
    let locals = compile(&s, "locals", LOCALS, &["-fpie"]);
    let definitions = compile(&s, "definitions", DEFINITIONS, &["-fpie"]);
    for inputs in [
        [
            Input::Object(&start),
            Input::Object(&locals),
            Input::Object(&definitions),
        ],
        [
            Input::Object(&definitions),
            Input::Object(&locals),
            Input::Object(&start),
        ],
    ] {
        let linked = class11::tls::link(&inputs, b"_start", &[]).unwrap();
        assert_eq!(run_image(&s, &linked.bytes), 42, "{}", linked.map);
        assert!(linked.map.starts_with("TINYLINK PIE MAP 1\n"));
        // `remote` is reached through one initial-exec GOT slot.
        assert_eq!(
            synthetic_size(&linked.map, ".got"),
            Some(8),
            "{}",
            linked.map
        );
        assert!(map_line(&linked.map, ".tls.image").is_some());
        assert!(map_line(&linked.map, ".tls.area").is_some());
        let headers = program_headers(&linked.bytes);
        assert_eq!(headers.len(), 5);
        let tls = headers[4];
        assert_eq!((tls.0, tls.1, tls.5), (class11::tls::PT_TLS, 4, 8));
        assert_eq!(u16at(&linked.bytes, 16), 3);
        assert_eq!(class11::tls::link(&inputs, b"_start", &[]).unwrap(), linked);
    }
}
const SPAWN: &str = concat!(
    ".text\n.globl spawn\n",
    "# int spawn(void *stack_top, void *thread_pointer, void (*function)(void))\n",
    "spawn: mov %rdx,%r9; mov %rsi,%r8; mov %rdi,%rsi\n",
    "mov $0xd0f00,%edi # VM|FS|FILES|SIGHAND|THREAD|SYSVSEM|SETTLS\n",
    "xor %edx,%edx; xor %r10d,%r10d; mov $56,%eax; syscall\n",
    "test %rax,%rax; jnz 1f\n",
    "call *%r9; xor %edi,%edi; mov $60,%eax; syscall\n",
    "1: ret\n",
);
const THREADS: &str = r#"
typedef unsigned long u64;
typedef unsigned int u32;
typedef unsigned char u8;
struct phdr { u32 type, flags; u64 offset, vaddr, paddr, filesz, memsz, align; };
extern int spawn(void *stack_top, void *thread_pointer, void (*function)(void));
extern int global;
__thread int counter = 5;
__thread long zeroed;
__thread int *tls_pointer = &global;
static u8 block[8192] __attribute__((aligned(64)));
static u8 stack[65536] __attribute__((aligned(16)));
static volatile int done;
static volatile int seen;
static int *volatile worker_counter;
static void worker(void) {
    int ok = counter == 5 && zeroed == 0 && *tls_pointer == 7;
    counter = 99;
    worker_counter = &counter;
    seen = ok ? 1 : 2;
    done = 1;
}
int check(u64 *sp) {
    u64 *p = sp + 1 + sp[0] + 1;
    while (*p) p++;
    const struct phdr *ph = 0;
    u64 phnum = 0;
    for (p++; p[0]; p += 2) {
        if (p[0] == 3) ph = (const struct phdr *)p[1];
        if (p[0] == 5) phnum = p[1];
    }
    if (!ph) return 1;
    const struct phdr *tls = 0;
    u64 bias = 0;
    int mapped = 0;
    for (u64 i = 0; i < phnum; i++) {
        if (ph[i].type == 7) tls = &ph[i];
        if (ph[i].type == 1 && ph[i].offset <= 64 && 64 < ph[i].offset + ph[i].filesz) {
            bias = (u64)ph - (ph[i].vaddr + 64 - ph[i].offset);
            mapped = 1;
        }
    }
    if (!tls || !mapped) return 2;
    u64 align = tls->align;
    u64 tp_offset = (tls->memsz + align - 1) / align * align;
    u64 start = ((u64)block + align - 1) / align * align;
    if (start + tp_offset + 8 > (u64)block + sizeof block) return 3;
    const u8 *template = (const u8 *)(bias + tls->vaddr);
    for (u64 i = 0; i < tls->filesz; i++) ((u8 *)start)[i] = template[i];
    u64 tp = start + tp_offset;
    *(u64 *)tp = tp;
    counter = 6;
    if (spawn(stack + sizeof stack, (void *)tp, worker) <= 0) return 4;
    for (long spins = 0; !done; spins++) {
        if (spins > 400000000) return 5;
        __asm__ volatile("pause");
    }
    if (seen != 1) return 6;
    if (counter != 6) return 7;
    if (worker_counter == &counter || *worker_counter != 99) return 8;
    return 42;
}
"#;
#[test]
fn threads_receive_independent_blocks_from_pt_tls() {
    let s = Scratch::new();
    let start = s.object(START);
    let spawn = s.object(SPAWN);
    let threads = compile(&s, "threads", THREADS, &["-fpie"]);
    let definitions = compile(&s, "definitions", DEFINITIONS, &["-fpie"]);
    let inputs = [
        Input::Object(&start),
        Input::Object(&spawn),
        Input::Object(&threads),
        Input::Object(&definitions),
    ];
    let linked = class11::tls::link(&inputs, b"_start", &[]).unwrap();
    for _ in 0..3 {
        assert_eq!(run_image(&s, &linked.bytes), 42, "{}", linked.map);
    }
}
const GOT_USER: &str = r#"
extern int shared_value;
extern int helper(void);
extern char constant[];
extern int maybe __attribute__((weak));
int check(void) {
    int (*volatile f)(void) = helper;
    if (&maybe != 0) return 1;
    if ((long)constant != 42) return 2;
    if (shared_value != 30) return 3;
    shared_value = 40;
    return f() + 2;
}
"#;
const GOT_PROVIDER: &str = ".text\n.globl helper\nhelper: mov shared_value(%rip),%eax; ret\n.data\n.globl shared_value\nshared_value: .long 30\n.globl constant\n.set constant,42\n";
#[test]
fn got_slots_relaxation_and_weak_references() {
    let s = Scratch::new();
    let start = s.object(START);
    let provider = s.object(GOT_PROVIDER);
    for (flags, slots) in [
        (&["-fPIC"][..], 2),
        (&["-fPIC", "-Wa,-mrelax-relocations=no"][..], 4),
    ] {
        let user = compile(&s, "user", GOT_USER, flags);
        let inputs = [
            Input::Object(&start),
            Input::Object(&user),
            Input::Object(&provider),
        ];
        let linked = class11::tls::link(&inputs, b"_start", &[]).unwrap();
        for _ in 0..2 {
            assert_eq!(
                run_image(&s, &linked.bytes),
                42,
                "{flags:?}\n{}",
                linked.map
            );
        }
        assert_eq!(
            synthetic_size(&linked.map, ".got"),
            Some(8 * slots),
            "{flags:?}\n{}",
            linked.map
        );
        assert!(map_line(&linked.map, ".tls.image").is_none());
        assert_eq!(program_headers(&linked.bytes).len(), 4);
    }
}
#[test]
fn inputs_without_got_or_tls_match_class9() {
    let s = Scratch::new();
    let start = s.object(concat!(
        ".section .text.start,\"ax\",@progbits\n.globl _start\n_start: call *function(%rip); mov %eax,%edi; mov $60,%eax; syscall\n",
        ".section .data.pointer,\"aw\",@progbits\nfunction: .quad answer\n",
        ".section .text.unused,\"ax\",@progbits\nunused: call missing\n",
        ".comm shared,8,8\n",
    ));
    let library = archive(
        &s,
        &[(
            "answer.o",
            ".section .text.answer,\"ax\",@progbits\n.globl answer\nanswer: movq $1,shared(%rip); mov $42,%eax; ret\n.comm shared,8,8\n",
        )],
    );
    let inputs = [Input::Object(&start), Input::Archive(&library)];
    let ours = class11::tls::link(&inputs, b"_start", &[]).unwrap();
    assert_eq!(ours, class11::pie::link(&inputs, b"_start", &[]).unwrap());
    assert_eq!(run_image(&s, &ours.bytes), 42);
    assert!(!ours.map.contains("SYNTHETIC"));
}
#[test]
fn rejects_dynamic_tls_and_mismatched_references() {
    use class11::tls::{Error, ReferenceError};
    let s = Scratch::new();
    let start = s.object(concat!(
        ".section .text.start,\"ax\",@progbits\n.globl _start\n_start: call get; mov $60,%eax; syscall\n",
        ".text\n.globl __tls_get_addr\n__tls_get_addr: ret\n",
    ));
    let dynamic = compile(
        &s,
        "dynamic",
        "__thread int v = 1;\nint get(void) { return v; }\n",
        &["-fPIC"],
    );
    assert!(matches!(
        class11::tls::link(
            &[Input::Object(&start), Input::Object(&dynamic)],
            b"_start",
            &[]
        ),
        Err(Error::Reference {
            kind: 19 | 20,
            cause: ReferenceError::UnsupportedTls,
            ..
        })
    ));
    let pc_relative = s.object(concat!(
        ".section .tbss,\"awT\",@nobits\nx: .zero 4\n",
        ".text\n.globl _start\n_start: lea x(%rip),%rax; ret\n",
    ));
    assert!(matches!(
        class11::tls::link(&[Input::Object(&pc_relative)], b"_start", &[]),
        Err(Error::Reference {
            kind: 2,
            cause: ReferenceError::TlsMismatch,
            ..
        })
    ));
    let local_exec = s.object(".text\n.globl _start\n_start: mov %fs:plain@tpoff,%eax; ret\n");
    let plain = s.object(".data\n.globl plain\nplain: .long 0\n");
    assert!(matches!(
        class11::tls::link(
            &[Input::Object(&local_exec), Input::Object(&plain)],
            b"_start",
            &[]
        ),
        Err(Error::Reference {
            kind: 23,
            cause: ReferenceError::TlsMismatch,
            ..
        })
    ));
    // References from dead code are never lowered, so they cannot fail.
    let dead = s.object(concat!(
        ".text\n.globl _start\n_start: ret\n",
        ".section .text.dead,\"ax\",@progbits\nlea y(%rip),%rax; mov %fs:y@tpoff,%eax\n",
        ".section .tbss,\"awT\",@nobits\ny: .zero 4\n",
    ));
    assert!(class11::tls::link(&[Input::Object(&dead)], b"_start", &[]).is_ok());
}
#[test]
fn cli_map_and_program_headers() {
    let s = Scratch::new();
    fs::write(s.0.join("start.o"), s.object(START)).unwrap();
    fs::write(
        s.0.join("locals.o"),
        compile(&s, "locals", LOCALS, &["-fpie"]),
    )
    .unwrap();
    fs::write(
        s.0.join("definitions.o"),
        compile(&s, "definitions", DEFINITIONS, &["-fpie"]),
    )
    .unwrap();
    let output = s.0.join("program");
    let map = s.0.join("program.map");
    let p = Command::new(env!("CARGO_BIN_EXE_class11"))
        .arg("-o")
        .arg(&output)
        .arg("-Map")
        .arg(&map)
        .args(["start.o", "locals.o", "definitions.o"].map(|n| s.0.join(n)))
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert_eq!(execute(&output), 42);
    let map = fs::read_to_string(map).unwrap();
    for name in [".got", ".tls.image", ".tls.area"] {
        assert!(map_line(&map, name).is_some(), "{map}");
    }
    let readelf = Command::new("readelf")
        .arg("-lW")
        .arg(&output)
        .output()
        .unwrap();
    assert!(readelf.status.success());
    let text = String::from_utf8(readelf.stdout).unwrap();
    assert!(text.contains("DYN") && text.contains(" TLS "), "{text}");
}

#[test]
fn merged_constants_tls_and_got_share_one_pie_layout() {
    let s = Scratch::new();
    let object = s.object(
        r#"
.text
.globl _start
_start:
    mov %fs:variable@tpoff,%edi
    mov value_a@GOTPCREL(%rip),%rax
    cmp %rax,pointer(%rip)
    jne failed
    mov value_a(%rip),%eax
    add %eax,%edi
    mov $60,%eax
    syscall
failed:
    mov $99,%edi
    mov $60,%eax
    syscall
.section .rodata.values,"aM",@progbits,4
.globl value_a,value_b
value_a: .long 21
value_b: .long 21
.section .data.pointer,"aw",@progbits
pointer: .quad value_b
.section .tdata.variable,"awT",@progbits
variable: .long 21
"#,
    );
    let inputs = [Input::Object(&object)];
    let prepared =
        class11::gc::prepare_with(&inputs, b"_start", &[], class11::tls::policy).unwrap();
    let lowered = class11::tls::lower_prepared(&prepared).unwrap();
    assert!(lowered.tls.is_some());
    assert_eq!(lowered.merged.groups.len(), 1);
    assert_eq!(lowered.merged.groups[0].bytes, 21u32.to_le_bytes());
    let merged_id = prepared
        .split()
        .iter()
        .find(|s| s.flags & 0x10 != 0)
        .unwrap()
        .id;
    assert_eq!(
        class11::unwind::locate(&lowered.merged, merged_id, 0).unwrap(),
        (0, 0)
    );
    assert_eq!(
        class11::unwind::locate(&lowered.merged, merged_id, 4).unwrap(),
        (0, 0)
    );
    assert!(!lowered.relative.is_empty());
    let linked = lowered.emit().unwrap();
    for _ in 0..3 {
        assert_eq!(run_image(&s, &linked.bytes), 42);
    }
    assert!(linked.map.contains(".rodata.cst4.1"));
    assert_eq!(
        u16::from_le_bytes(linked.bytes[16..18].try_into().unwrap()),
        3
    );
    assert_eq!(
        u16::from_le_bytes(linked.bytes[56..58].try_into().unwrap()),
        5
    );
}

#[test]
fn merged_section_symbol_addends_follow_input_pieces() {
    let s = Scratch::new();
    let object = s.object(
        r#"
.text
.globl _start
_start:
    mov %fs:variable@tpoff,%edi
    lea .rodata.values(%rip),%rax
    cmp %rax,pointer(%rip)
    jne failed
    mov $60,%eax
    syscall
failed:
    mov $99,%edi
    mov $60,%eax
    syscall
.section .rodata.values,"aM",@progbits,4
.long 21
.long 21
.section .data.pointer,"aw",@progbits
pointer: .quad .rodata.values + 4
.section .tdata.variable,"awT",@progbits
variable: .long 42
"#,
    );
    let inputs = [Input::Object(&object)];
    // A PC32 SECTION reference with encoding addend -4 is outside the mapped
    // piece. Reject it instead of silently treating the addend as pure offset.
    assert!(matches!(
        class11::tls::link(&inputs, b"_start", &[]),
        Err(class11::tls::Error::Merge(class11::unwind::Error::Merge {
            cause: class11::unwind::MergeError::InvalidReference(_),
            ..
        }))
    ));
    let object = s.object(
        r#"
.text
.globl _start
_start:
    mov %fs:variable@tpoff,%edi
    mov pointer(%rip),%rax
    cmpl $21,(%rax)
    jne failed
    mov $60,%eax
    syscall
failed:
    mov $99,%edi
    mov $60,%eax
    syscall
.section .rodata.values,"aM",@progbits,4
.long 21
.long 21
.section .data.pointer,"aw",@progbits
pointer: .quad .rodata.values + 4
.section .tdata.variable,"awT",@progbits
variable: .long 42
"#,
    );
    let inputs = [Input::Object(&object)];
    let prepared =
        class11::gc::prepare_with(&inputs, b"_start", &[], class11::tls::policy).unwrap();
    let lowered = class11::tls::lower_prepared(&prepared).unwrap();
    assert_eq!(lowered.merged.groups[0].bytes.len(), 4);
    let address = lowered.image.placements[&(usize::MAX, class11::unwind::MERGE_BASE)].address
        - class11::executable::BASE;
    assert_eq!(lowered.relative.len(), 1);
    assert_eq!(lowered.relative[0].addend, address as i64);
    let linked = lowered.emit().unwrap();
    assert_eq!(run_image(&s, &linked.bytes), 42);
}

#[test]
fn pie_tls_and_unwind_headers_describe_the_same_layout() {
    let s = Scratch::new();
    for tls in [false, true] {
        let object = s.object(&format!(
            r#"
.text
.globl _start
_start:
.cfi_startproc
{load}
mov $60,%eax
syscall
.cfi_endproc
{data}
"#,
            load = if tls {
                "mov %fs:variable@tpoff,%edi"
            } else {
                "mov $42,%edi"
            },
            data = if tls {
                ".section .tdata.variable,\"awT\",@progbits\nvariable: .long 42"
            } else {
                ""
            }
        ));
        let inputs = [Input::Object(&object)];
        let prepared =
            class11::gc::prepare_with(&inputs, b"_start", &[], class11::tls::policy).unwrap();
        let lowered = class11::tls::lower_prepared(&prepared).unwrap();
        let hdr = &lowered.image.placements[&class11::tls::EH_FRAME_HDR];
        let expected = (
            hdr.file_offset.unwrap(),
            hdr.address - class11::executable::BASE,
            hdr.size,
        );
        let frame = &lowered.image.placements[&class11::tls::EH_FRAME];
        let frame_rva = frame.address - class11::executable::BASE;
        let entry = lowered.entry_rva;
        let linked = lowered.emit().unwrap();
        let u64at = |at: usize| u64::from_le_bytes(linked.bytes[at..at + 8].try_into().unwrap());
        let count = u16::from_le_bytes(linked.bytes[56..58].try_into().unwrap());
        assert_eq!(count, if tls { 6 } else { 5 });
        let at = class11::layout::HEADER_BYTES as usize + if tls { 56 } else { 0 };
        assert_eq!(
            u32::from_le_bytes(linked.bytes[at..at + 4].try_into().unwrap()),
            class11::unwind::PT_GNU_EH_FRAME
        );
        assert_eq!((u64at(at + 8), u64at(at + 16), u64at(at + 32)), expected);
        let h = expected.0 as usize;
        assert_eq!(&linked.bytes[h..h + 4], &[1, 0x1b, 0x03, 0x3b]);
        let relative = |offset: usize, base: u64| {
            (i128::from(base)
                + i128::from(i32::from_le_bytes(
                    linked.bytes[offset..offset + 4].try_into().unwrap(),
                ))) as u64
        };
        assert_eq!(relative(h + 4, expected.1 + 4), frame_rva);
        assert_eq!(
            u32::from_le_bytes(linked.bytes[h + 8..h + 12].try_into().unwrap()),
            1
        );
        assert_eq!(relative(h + 12, expected.1), entry);
        for _ in 0..3 {
            assert_eq!(run_image(&s, &linked.bytes), 42);
        }
        assert!(linked.map.contains(".eh_frame_hdr"));
        let mut invalid = class11::tls::lower_prepared(&prepared).unwrap();
        invalid
            .image
            .placements
            .get_mut(&class11::tls::UNWIND_PHDR)
            .unwrap()
            .size = 0;
        assert_eq!(
            invalid.emit().err(),
            Some(class11::tls::Error::InvalidReservation)
        );
    }
}
