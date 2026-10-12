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
            "tinylink-class10-{}-{}",
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
use class10::archive::Input;
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
/// Compile freestanding C. `unwind` selects whether GCC emits `.eh_frame`.
fn compile(s: &Scratch, name: &str, source: &str, extra: &[&str], unwind: bool) -> Vec<u8> {
    let input = s.0.join(format!("{name}.c"));
    let output = s.0.join(format!("{name}.o"));
    fs::write(&input, source).unwrap();
    let tables: &[&str] = if unwind {
        &["-fasynchronous-unwind-tables"]
    } else {
        &["-fno-asynchronous-unwind-tables", "-fno-unwind-tables"]
    };
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
        .args(tables)
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
/// Decoded `.eh_frame_hdr` table as (initial location, FDE address, FDE range).
fn search_table(elf: &[u8]) -> Vec<(u64, u64, u64)> {
    let headers = program_headers(elf);
    let &(_, flags, offset, address, size, align) = headers
        .iter()
        .find(|h| h.0 == class10::unwind::PT_GNU_EH_FRAME)
        .expect("PT_GNU_EH_FRAME");
    assert_eq!((flags, align), (4, 4));
    let file = |vaddr: u64| -> usize {
        let load = headers
            .iter()
            .find(|h| h.0 == 1 && h.3 <= vaddr && vaddr < h.3 + h.4)
            .expect("address inside a file-backed PT_LOAD");
        (load.2 + (vaddr - load.3)) as usize
    };
    assert_eq!(file(address), offset as usize);
    let hdr = &elf[offset as usize..(offset + size) as usize];
    assert_eq!(&hdr[..4], &[1, 0x1b, 0x03, 0x3b]);
    let count = u32at(hdr, 8) as usize;
    assert_eq!(size as usize, 12 + 8 * count);
    let at = |i: usize| (address as i64 + i64::from(u32at(hdr, i) as i32)) as u64;
    (0..count)
        .map(|i| {
            let (initial, fde) = (at(12 + 8 * i), at(16 + 8 * i));
            let record = file(fde);
            let field = fde + 8;
            let begin = (field as i64 + i64::from(u32at(elf, record + 8) as i32)) as u64;
            assert_eq!(begin, initial, "table and FDE disagree");
            (initial, fde, u64::from(u32at(elf, record + 12)))
        })
        .collect()
}
const START: &str = ".section .text.start,\"ax\",@progbits\n.globl _start\n_start: mov %rsp,%rdi; call check; mov %eax,%edi; mov $60,%eax; syscall\n";
const MERGE_A: &str = r#"
extern const char *other_greeting(void);
extern const char *other_long(void);
extern double other_scale(double);
static const char *const table[] = { "hello, merged world" + 7, "tail" };
__attribute__((noipa)) const char *greeting(void) { return "hello, merged world"; }
__attribute__((noipa)) const char *long_text(void) {
    return "a fairly long string literal that exceeds thirty two bytes";
}
static inline __attribute__((always_inline)) int same(const char *a, const char *b) {
    while (*a && *a == *b) { a++; b++; }
    return *a == *b;
}
int check(void) {
    if (greeting() != other_greeting()) return 1;
    if (long_text() != other_long()) return 2;
    if (!same(table[0], "merged world")) return 3;
    if (table[0] != greeting() + 7) return 4;
    if (!same(table[1], "tail")) return 5;
    if (other_scale(2.0) != 6.5) return 6;
    return 42;
}
"#;
const MERGE_B: &str = r#"
__attribute__((noipa)) const char *other_greeting(void) { return "hello, merged world"; }
__attribute__((noipa)) const char *other_long(void) {
    return "a fairly long string literal that exceeds thirty two bytes";
}
__attribute__((noipa)) double other_scale(double x) { return x * 3.25; }
"#;
/// Sum of input sizes of map rows whose names contain `needle`, and the size
/// of the synthetic group named `group`.
fn map_sizes(map: &str, needle: &str, group: &str) -> (u64, u64) {
    let size = |line: &str| line.split(' ').nth(3).unwrap().parse::<u64>().unwrap();
    let inputs = map
        .lines()
        .filter(|l| l.starts_with("KEEP ") && l.contains(needle))
        .map(size)
        .sum();
    let synthetic = map
        .lines()
        .find(|l| l.starts_with("SYNTHETIC ") && l.ends_with(&format!(" {group}")))
        .map(|l| l.split(' ').nth(2).unwrap().parse().unwrap())
        .unwrap_or_else(|| panic!("no SYNTHETIC {group} in\n{map}"));
    (inputs, synthetic)
}
#[test]
fn merged_constants_have_one_address() {
    let s = Scratch::new();
    let start = s.object(START);
    for model in ["-fno-pie", "-fpie"] {
        let a = compile(&s, "a", MERGE_A, &[model], false);
        let b = compile(&s, "b", MERGE_B, &[model], false);
        for inputs in [
            [Input::Object(&start), Input::Object(&a), Input::Object(&b)],
            [Input::Object(&b), Input::Object(&a), Input::Object(&start)],
        ] {
            let linked = class10::unwind::link(&inputs, b"_start", &[]).unwrap();
            assert_eq!(run_image(&s, &linked.bytes), 42, "{model}\n{}", linked.map);
            let (inputs_size, merged) = map_sizes(&linked.map, ".str1.1", ".rodata.str1.1");
            assert!(merged < inputs_size, "{}", linked.map);
            let (inputs_size, merged) = map_sizes(&linked.map, ".str1.8", ".rodata.str1.8");
            assert!(merged < inputs_size, "{}", linked.map);
            map_sizes(&linked.map, ".cst8", ".rodata.cst8.8");
            assert!(!program_headers(&linked.bytes)
                .iter()
                .any(|h| h.0 == class10::unwind::PT_GNU_EH_FRAME));
            assert_eq!(
                class10::unwind::link(&inputs, b"_start", &[]).unwrap(),
                linked
            );
        }
    }
}
const SELF_CHECK: &str = r#"
typedef unsigned long u64;
typedef unsigned int u32;
typedef int i32;
typedef unsigned char u8;
struct phdr { u32 type, flags; u64 offset, vaddr, paddr, filesz, memsz, align; };
__attribute__((noipa)) int leaf(int x) { return x * 3 + 1; }
__attribute__((noipa)) int unused_function(int x) { return x - 7; }
static inline __attribute__((always_inline)) int covered(const u8 *hdr, const i32 *table, u32 count, const u8 *eh, u64 pc) {
    u32 low = 0, high = count;
    while (high - low > 1) {
        u32 mid = low + (high - low) / 2;
        if ((u64)(hdr + table[2 * mid]) <= pc) low = mid; else high = mid;
    }
    if (count == 0 || (u64)(hdr + table[2 * low]) > pc) return 5;
    const u8 *fde = hdr + table[2 * low + 1];
    if (fde < eh) return 6;
    const u8 *cie = fde + 4 - *(const i32 *)(fde + 4);
    if (*(const u32 *)(cie + 4) != 0 || cie[8] != 1 || cie[9] != 'z' || cie[10] != 'R' || cie[11] != 0)
        return 7;
    u64 begin = (u64)(fde + 8) + *(const i32 *)(fde + 8);
    if (begin != (u64)(hdr + table[2 * low])) return 8;
    if (pc < begin || pc >= begin + *(const u32 *)(fde + 12)) return 9;
    return 0;
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
    if (!ph || !phnum) return 1;
    const u8 *hdr = 0;
    for (u64 i = 0; i < phnum; i++)
        if (ph[i].type == 0x6474e550) hdr = (const u8 *)ph[i].vaddr;
    if (!hdr) return 2;
    if (hdr[0] != 1 || hdr[1] != 0x1b || hdr[2] != 0x03 || hdr[3] != 0x3b) return 3;
    const u8 *eh = hdr + 4 + *(const i32 *)(hdr + 4);
    u32 count = *(const u32 *)(hdr + 8);
    const i32 *table = (const i32 *)(hdr + 12);
    for (u32 i = 1; i < count; i++)
        if (table[2 * i] <= table[2 * i - 2]) return 4;
    u64 targets[] = { (u64)check, (u64)leaf, (u64)check + 1 };
    for (u32 i = 0; i < 3; i++) {
        int bad = covered(hdr, table, count, eh, targets[i]);
        if (bad) return bad;
    }
    if (count != EXPECTED) return 10;
    return leaf(41) == 124 ? 42 : 11;
}
"#;
#[test]
fn runtime_finds_live_fdes_through_its_program_headers() {
    let s = Scratch::new();
    let start = s.object(START);
    let check = compile(&s, "check", SELF_CHECK, &["-fno-pie", "-DEXPECTED=2"], true);
    let inputs = [Input::Object(&start), Input::Object(&check)];
    let linked = class10::unwind::link(&inputs, b"_start", &[]).unwrap();
    assert_eq!(run_image(&s, &linked.bytes), 42, "{}", linked.map);
    let headers = program_headers(&linked.bytes);
    assert_eq!(headers.len(), 5);
    assert_eq!(headers[4].0, class10::unwind::PT_GNU_EH_FRAME);
    assert_eq!(search_table(&linked.bytes).len(), 2);
    assert!(linked
        .map
        .lines()
        .any(|l| l.starts_with("DROP ") && l.ends_with(" .text.unused_function")));
    for name in [".eh_frame", ".eh_frame_hdr"] {
        assert!(linked
            .map
            .lines()
            .any(|l| l.starts_with("SYNTHETIC ") && l.ends_with(&format!(" {name}"))));
    }
    let readelf = Command::new("readelf")
        .arg("-lW")
        .arg(s.0.join("program"))
        .output()
        .unwrap();
    assert!(readelf.status.success());
    assert!(String::from_utf8(readelf.stdout)
        .unwrap()
        .contains("GNU_EH_FRAME"));
    // The fixture itself is validated against GNU ld's unwind index.
    fs::write(s.0.join("start.o"), &start).unwrap();
    fs::write(s.0.join("check.o"), &check).unwrap();
    let reference = s.0.join("reference");
    let p = Command::new("ld")
        .args([
            "-static",
            "--gc-sections",
            "--eh-frame-hdr",
            "-e",
            "_start",
            "-o",
        ])
        .arg(&reference)
        .arg(s.0.join("start.o"))
        .arg(s.0.join("check.o"))
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert_eq!(execute(&reference), 42);
    let gnu = search_table(&fs::read(&reference).unwrap());
    let ours = search_table(&linked.bytes);
    let ranges = |t: &[(u64, u64, u64)]| t.iter().map(|e| e.2).collect::<Vec<_>>();
    let mut gnu_ranges = ranges(&gnu);
    let mut our_ranges = ranges(&ours);
    gnu_ranges.sort();
    our_ranges.sort();
    assert_eq!(our_ranges, gnu_ranges);
    // Keeping the unused function keeps its FDE as well.
    let kept = compile(&s, "kept", SELF_CHECK, &["-fno-pie", "-DEXPECTED=3"], true);
    let linked = class10::unwind::link(
        &[Input::Object(&start), Input::Object(&kept)],
        b"_start",
        &[b"unused_function"],
    )
    .unwrap();
    assert_eq!(run_image(&s, &linked.bytes), 42);
}
#[test]
fn dead_merge_sections_and_fdes_contribute_nothing() {
    let s = Scratch::new();
    let object = s.object(concat!(
        ".section .text.start,\"ax\",@progbits\n.globl _start\n_start:\n.cfi_startproc\n",
        "lea live(%rip),%rsi; cmpb $'l',(%rsi); jne 1f; mov $42,%edi; jmp 2f\n1: mov $3,%edi\n2: mov $60,%eax; syscall\n.cfi_endproc\n",
        ".section .text.dead,\"ax\",@progbits\ndead:\n.cfi_startproc\nlea gone(%rip),%rax; ret\n.cfi_endproc\n",
        ".section .rodata.live,\"aMS\",@progbits,1\nlive: .asciz \"live\"\n",
        ".section .rodata.gone,\"aMS\",@progbits,1\ngone: .asciz \"gone\"\n",
    ));
    let linked = class10::unwind::link(&[Input::Object(&object)], b"_start", &[]).unwrap();
    assert_eq!(run_image(&s, &linked.bytes), 42);
    assert!(linked
        .map
        .lines()
        .any(|l| l.starts_with("DROP ") && l.ends_with(" .rodata.gone")));
    assert_eq!(
        map_sizes(&linked.map, ".rodata.live", ".rodata.str1.1"),
        (5, 5)
    );
    assert_eq!(search_table(&linked.bytes).len(), 1);
    assert!(linked
        .map
        .lines()
        .any(|l| l.starts_with("KEEP ") && l.ends_with(" .eh_frame")));
}
#[test]
fn lowering_retains_merge_mapping_before_serialization() {
    let s = Scratch::new();
    let object = s.object(concat!(
        ".section .text.start,\"ax\",@progbits\n.globl _start\n_start:\n.cfi_startproc\n",
        "mov value_copy(%rip),%edi; mov $60,%eax; syscall\n.cfi_endproc\n",
        ".section .rodata.values,\"aM\",@progbits,4\n",
        ".globl value\nvalue: .long 42\n.globl value_copy\nvalue_copy: .long 42\n",
    ));
    let inputs = [Input::Object(&object)];
    let prepared =
        class10::gc::prepare_with(&inputs, b"_start", &[], class10::unwind::policy).unwrap();
    assert_eq!(prepared.split().len(), 1);
    let input = &prepared.split()[0];
    let before = input.bytes.to_vec();
    let lowered = class10::unwind::lower_prepared(&prepared).unwrap();

    // No ELF header has been serialized. The planned output already contains
    // deduplicated constants, relocated instructions, and rebuilt unwind data.
    assert!(lowered.image.bytes[..64].iter().all(|&b| b == 0));
    assert_eq!(lowered.merged.groups.len(), 1);
    assert_eq!(lowered.merged.groups[0].bytes, 42u32.to_le_bytes());
    assert_eq!(
        class10::unwind::locate(&lowered.merged, input.id, 0).unwrap(),
        (0, 0)
    );
    assert_eq!(
        class10::unwind::locate(&lowered.merged, input.id, 4).unwrap(),
        (0, 0)
    );
    assert_eq!(input.bytes, before);
    assert!(lowered
        .image
        .placements
        .contains_key(&class10::unwind::EH_FRAME));
    assert!(lowered
        .image
        .placements
        .contains_key(&class10::unwind::EH_FRAME_HDR));

    let linked = lowered.emit().unwrap();
    assert_eq!(
        u16::from_le_bytes(linked.bytes[56..58].try_into().unwrap()),
        5
    );
    assert_eq!(search_table(&linked.bytes).len(), 1);
    assert_eq!(run_image(&s, &linked.bytes), 42);
    assert_eq!(
        linked,
        class10::unwind::link(&inputs, b"_start", &[]).unwrap()
    );

    let mut invalid = class10::unwind::lower_prepared(&prepared).unwrap();
    invalid
        .image
        .placements
        .get_mut(&class10::unwind::PHDR)
        .unwrap()
        .size = 0;
    assert_eq!(
        invalid.emit().err(),
        Some(class10::unwind::Error::InvalidReservation)
    );
}

#[test]
fn inputs_without_new_features_match_class8() {
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
    let ours = class10::unwind::link(&inputs, b"_start", &[]).unwrap();
    assert_eq!(ours, class10::gc::link(&inputs, b"_start", &[]).unwrap());
    assert_eq!(run_image(&s, &ours.bytes), 42);
    assert!(!ours.map.contains("SYNTHETIC"));
}
#[test]
fn rejects_unsupported_merge_and_unwind_inputs() {
    use class10::unwind::{Error, MergeError, UnwindError};
    let s = Scratch::new();
    let link = |source: &str| {
        let object = s.object(source);
        class10::unwind::link(&[Input::Object(&object)], b"_start", &[])
    };
    let entry = ".section .text.start,\"ax\",@progbits\n.globl _start\n_start: lea target(%rip),%rax; ret\n";
    assert!(matches!(
        link(&format!(
            "{entry}.section .rodata.m,\"aM\",@progbits,8\ntarget: .quad _start\n"
        )),
        Err(Error::Merge {
            cause: MergeError::Relocated,
            ..
        })
    ));
    assert!(matches!(
        link(&format!(
            "{entry}.section .rodata.w,\"aMS\",@progbits,2\ntarget: .short 65,0\n"
        )),
        Err(Error::Merge {
            cause: MergeError::Unsupported,
            ..
        })
    ));
    assert!(matches!(
        link(&format!("{entry}.section .rodata.s,\"aMS\",@progbits,1\n.asciz \"ab\"\n.data\ntarget: .quad .rodata.s+3\n")),
        Err(Error::Merge { cause: MergeError::InvalidReference(3), .. })
    ));
    assert!(matches!(
        link(concat!(
            ".section .text.start,\"ax\",@progbits\n.globl _start\n_start:\n.cfi_startproc\n.cfi_personality 0x3,personality\nret\n.cfi_endproc\n",
            ".data\npersonality: .quad 0\n",
        )),
        Err(Error::Unwind { cause: UnwindError::Unsupported(0), .. })
    ));
    assert!(matches!(
        link(&format!("{entry}.section .data.code,\"aw\",@progbits\ntarget:\n.cfi_startproc\nret\n.cfi_endproc\n")),
        Err(Error::FdeTarget { offset: 24, .. })
    ));
}
#[test]
fn cli_writes_merge_and_unwind_map() {
    let s = Scratch::new();
    fs::write(s.0.join("start.o"), s.object(START)).unwrap();
    let a = compile(&s, "a", MERGE_A, &["-fno-pie"], true);
    let b = compile(&s, "b", MERGE_B, &["-fno-pie"], true);
    fs::write(s.0.join("a.o"), a).unwrap();
    fs::write(s.0.join("b.o"), b).unwrap();
    let output = s.0.join("program");
    let map = s.0.join("program.map");
    let p = Command::new(env!("CARGO_BIN_EXE_class10"))
        .arg("-o")
        .arg(&output)
        .arg("-Map")
        .arg(&map)
        .args(["start.o", "a.o", "b.o"].map(|n| s.0.join(n)))
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert_eq!(execute(&output), 42);
    let map = fs::read_to_string(map).unwrap();
    assert!(map.starts_with("TINYLINK MAP 1\n"));
    for name in [".rodata.str1.1", ".eh_frame", ".eh_frame_hdr"] {
        assert!(
            map.lines()
                .any(|l| l.starts_with("SYNTHETIC ") && l.ends_with(name)),
            "{map}"
        );
    }
    let table = search_table(&fs::read(&output).unwrap());
    assert_eq!(table.len(), 6, "{map}");
}

#[test]
fn folded_ordering_retains_merge_mapping_and_one_fde_per_copy() {
    use class10::gc::SectionId;
    use std::collections::BTreeMap;
    let s = Scratch::new();
    let object = s.object(
        r#"
.section .text.start,"ax",@progbits
.globl _start
_start:
.cfi_startproc
call twin_b
mov %eax,%edi
mov $60,%eax
syscall
.cfi_endproc
.section .text.a,"ax",@progbits
.globl twin_a
.type twin_a,@function
twin_a:
.cfi_startproc
mov value_copy(%rip),%eax
ret
.cfi_endproc
.section .text.b,"ax",@progbits
.globl twin_b
.type twin_b,@function
twin_b:
.cfi_startproc
mov value_copy(%rip),%eax
ret
.cfi_endproc
.section .rodata.values,"aM",@progbits,4
value: .long 42
value_copy: .long 42
"#,
    );
    let inputs = [Input::Object(&object)];
    let prepared =
        class10::gc::prepare_with(&inputs, b"_start", &[b"twin_a"], class10::unwind::policy)
            .unwrap();
    let section = |name: &[u8]| {
        prepared
            .map_rows()
            .iter()
            .find(|r| r.name == name)
            .unwrap()
            .id
    };
    let a = section(b".text.a");
    let b = section(b".text.b");
    let start = section(b".text.start");
    let aliases = BTreeMap::from([(b, a)]);
    let lowered = class10::unwind::lower_with(&prepared, &aliases, &[b, a]).unwrap();
    assert!(!lowered.image.placements.contains_key(&(b.file, b.section)));
    assert!(
        lowered.image.placements[&(a.file, a.section)].address
            < lowered.image.placements[&(start.file, start.section)].address
    );
    assert_eq!(lowered.merged.groups[0].bytes, 42u32.to_le_bytes());
    let retained = lowered.image.placements[&(a.file, a.section)].address;
    let linked = lowered.emit().unwrap();
    let table = search_table(&linked.bytes);
    assert_eq!(table.len(), 2, "folded copy must not leave a duplicate FDE");
    assert!(table.iter().any(|(address, _, _)| *address == retained));
    assert_eq!(run_image(&s, &linked.bytes), 42);
    let invalid = SectionId {
        file: usize::MAX,
        section: u16::MAX,
    };
    assert_eq!(
        class10::unwind::lower_with(&prepared, &aliases, &[invalid]).err(),
        Some(class10::unwind::Error::InvalidOrderSection(invalid))
    );
    // The entry may itself be one of the equivalent copies.
    let entry_prepared =
        class10::gc::prepare_with(&inputs, b"twin_b", &[b"twin_a"], class10::unwind::policy)
            .unwrap();
    let redirected = class10::unwind::lower_with(&entry_prepared, &aliases, &[]).unwrap();
    assert_eq!(
        redirected.entry_address,
        redirected.image.placements[&(a.file, a.section)].address
    );
}
