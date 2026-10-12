use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
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
            "tinylink-class15-{}-{}",
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
fn compile(s: &Scratch, name: &str, source: &str, flags: &[&str]) -> Vec<u8> {
    let input = s.0.join(format!("{name}.c"));
    let output = s.0.join(format!("{name}.o"));
    fs::write(&input, source).unwrap();
    let p = Command::new("cc")
        .args([
            "-c",
            "-ffreestanding",
            "-fno-tree-loop-distribute-patterns",
            "-ffunction-sections",
        ])
        .args([
            "-fdata-sections",
            "-fno-stack-protector",
            "-fcf-protection=none",
        ])
        .args(flags)
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    fs::read(output).unwrap()
}
fn archive(s: &Scratch, members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut paths = Vec::new();
    for (name, bytes) in members {
        let p = s.0.join(name);
        fs::write(&p, bytes).unwrap();
        paths.push(p);
    }
    let out = s.0.join("library.a");
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
/// Run a generated program; return its stdout and exit status.
fn capture(path: &std::path::Path) -> (String, i32) {
    let output = Command::new(path).output().unwrap();
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        output.status.code().expect("program died from a signal"),
    )
}
fn inputs<'a>(files: &[&'a [u8]]) -> Vec<class15::archive::Input<'a>> {
    files
        .iter()
        .map(|b| {
            if b.starts_with(b"!<arch>\n") {
                class15::archive::Input::Archive(b)
            } else {
                class15::archive::Input::Object(b)
            }
        })
        .collect()
}
const START: &str = ".section .text.start,\"ax\",@progbits\n.globl _start\n_start: mov %rsp,%rdi; call check; mov %eax,%edi; mov $231,%eax; syscall\n";
const STRINGS: &str = r#"__attribute__((noipa)) const char *greeting(void) { return "hello"; }
__attribute__((noipa)) double scale(double x) { return x * 3.25; }
int check(void) { return greeting()[1] == 'e' && scale(2.0) == 6.5 ? 42 : 1; }
"#;
const THREADS: &str = r#"__thread int counter = 5;
__thread long zeroed;
extern int helper(void);
int check(void) { counter += helper(); return counter + (int)zeroed; }
"#;
const HELPER: &str = "int helper(void) { return 37; }\n";
const TWINS: &str = r#"__attribute__((noipa)) int twin_a(int x) { return x * 3 + 1; }
__attribute__((noipa)) int twin_b(int x) { return x * 3 + 1; }
static int table[4] = {1, 2, 3, 4};
int check(void) { return twin_a(4) + twin_b(4) + table[3] + 12; }
"#;
/// Valid samples covering merge/unwind, TLS/GOT, debug/ICF, and archives.
fn corpus(s: &Scratch) -> Vec<Vec<Vec<u8>>> {
    let start = s.object(START);
    let library = archive(
        s,
        &[("helper.o", &compile(s, "helper", HELPER, &["-O1", "-fPIC"]))],
    );
    vec![
        vec![
            start.clone(),
            compile(s, "strings", STRINGS, &["-O1", "-fno-pie"]),
        ],
        vec![
            start.clone(),
            compile(
                s,
                "threads",
                THREADS,
                &["-O1", "-fpie", "-fno-asynchronous-unwind-tables"],
            ),
            library,
        ],
        vec![
            start,
            compile(
                s,
                "twins",
                TWINS,
                &["-g", "-Og", "-fno-pie", "-fno-asynchronous-unwind-tables"],
            ),
        ],
    ]
}
type Backend = (&'static str, fn(&[class15::archive::Input<'_>]) -> bool);
const BACKENDS: [Backend; 5] = [
    ("class10", |i| {
        class15::unwind::link(i, b"_start", &[]).is_ok()
    }),
    ("class11", |i| class15::tls::link(i, b"_start", &[]).is_ok()),
    ("class13", |i| {
        class15::icf::link(i, b"_start", &[], &[]).is_ok()
    }),
    ("class14", |i| class15::driver::link(i, b"_start").is_ok()),
    ("class13 PIE", |i| {
        class15::icf::link_pie(i, b"_start", &[], &[]).is_ok()
    }),
];
fn assert_no_panics(
    name: &str,
    corpus: &[Vec<Vec<u8>>],
    iterations: u64,
    seed: u64,
    link: fn(&[class15::archive::Input<'_>]) -> bool,
) {
    let run = |files: &[&[u8]]| link(&inputs(files));
    let report = class15::fuzz::survey(corpus, iterations, seed, &run);
    assert!(
        report.panics.is_empty(),
        "{name}: {} of {iterations} mutated links panicked (seed {seed}); first: {:?}",
        report.panics.len(),
        report.panics[0]
    );
    assert!(report.rejected > 0, "{name}: no mutation was rejected");
}
#[test]
fn backends_never_panic_on_mutated_objects() {
    let s = Scratch::new();
    let corpus = corpus(&s);
    // Each sample links with the backend that supports its features.
    for (sample, backend) in [(0, 0), (1, 1), (2, 2), (1, 3), (1, 4)] {
        let files: Vec<&[u8]> = corpus[sample].iter().map(Vec::as_slice).collect();
        assert!(
            (BACKENDS[backend].1)(&inputs(&files)),
            "sample {sample} with {}",
            BACKENDS[backend].0
        );
    }
    for (index, (name, link)) in BACKENDS.iter().enumerate() {
        assert_no_panics(name, &corpus, 1000, 0x5eed + index as u64, *link);
    }
}
#[test]
fn rust_objects_never_panic_when_mutated() {
    let s = Scratch::new();
    let source = s.0.join("main.rs");
    fs::write(&source, include_str!("fixtures/no_std_fmt.rs")).unwrap();
    let object = s.0.join("main.o");
    let p = Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "-C",
            "panic=abort",
            "-C",
            "opt-level=2",
            "-C",
            "force-unwind-tables=no",
        ])
        .args(["--emit=obj", "-o"])
        .arg(&object)
        .arg(&source)
        .output()
        .unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    let sysroot = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    let lib = std::path::PathBuf::from(String::from_utf8(sysroot.stdout).unwrap().trim())
        .join("lib/rustlib/x86_64-unknown-linux-gnu/lib");
    let mut sample = vec![fs::read(&object).unwrap()];
    for prefix in ["libcore-", "libcompiler_builtins-"] {
        let rlib = fs::read_dir(&lib)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                let name = p.file_name().unwrap().to_string_lossy();
                name.starts_with(prefix) && name.ends_with(".rlib")
            })
            .unwrap();
        sample.push(fs::read(rlib).unwrap());
    }
    let link = BACKENDS[3].1;
    let files: Vec<&[u8]> = sample.iter().map(Vec::as_slice).collect();
    assert!(link(&inputs(&files)));
    assert_no_panics("class14 rust", &[sample], 40, 0x7275_7374, link);
}
#[test]
fn outputs_are_reproducible() {
    let s = Scratch::new();
    let corpus = corpus(&s);
    for (sample, (name, _)) in [(0, BACKENDS[0]), (1, BACKENDS[1]), (2, BACKENDS[2])] {
        let files: Vec<&[u8]> = corpus[sample].iter().map(Vec::as_slice).collect();
        let link = |files: &[&[u8]]| match name {
            "class10" => class15::unwind::link(&inputs(files), b"_start", &[]).unwrap(),
            "class11" => class15::tls::link(&inputs(files), b"_start", &[]).unwrap(),
            _ => class15::icf::link(&inputs(files), b"_start", &[], &[]).unwrap(),
        };
        let first = link(&files);
        assert_eq!(link(&files), first, "{name} is not deterministic");
        // Copies of the same bytes at different addresses link identically.
        let copies: Vec<Vec<u8>> = files.iter().map(|f| f.to_vec()).collect();
        let copies: Vec<&[u8]> = copies.iter().map(Vec::as_slice).collect();
        assert_eq!(link(&copies), first, "{name} depends on buffer addresses");
        let path = s.0.join(format!("program-{name}"));
        fs::write(&path, &first.bytes).unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(capture(&path).1, 42, "{name}");
    }
}
const PROGRAMS: [&str; 3] = [
    "static int values[5] = {3, 1, 4, 1, 5};\nint check(void) { int s = 0; for (int i = 0; i < 5; i++) s += values[i] * (i + 1); return s - 4; }\n",
    "static int add(int a, int b) { return a + b; }\nstatic int sub(int a, int b) { return a - b; }\nint (*const ops[2])(int, int) = {add, sub};\nint check(void) { volatile int k = 0; return ops[k](40, 7) - ops[k + 1](5, 3) - 3; }\n",
    "long counter;\nlong bump(long by) { counter += by; return counter; }\nint check(void) { for (int i = 0; i < 6; i++) bump(i); return (int)(counter + 27); }\n",
];
#[test]
fn programs_behave_as_under_gnu_ld() {
    let s = Scratch::new();
    let start = s.object(START);
    fs::write(s.0.join("start.o"), &start).unwrap();
    for (index, source) in PROGRAMS.iter().enumerate() {
        let object = compile(
            &s,
            &format!("program{index}"),
            source,
            &["-O1", "-fno-pie", "-fno-asynchronous-unwind-tables"],
        );
        let object_path = s.0.join(format!("program{index}.o"));
        let ours = class15::icf::link(&inputs(&[&start, &object]), b"_start", &[], &[]).unwrap();
        let ours_path = s.0.join(format!("ours{index}"));
        fs::write(&ours_path, &ours.bytes).unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&ours_path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let gnu = s.0.join(format!("gnu{index}"));
        let p = Command::new("ld")
            .args(["-static", "--gc-sections", "-e", "_start", "-o"])
            .arg(&gnu)
            .arg(s.0.join("start.o"))
            .arg(&object_path)
            .output()
            .unwrap();
        assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
        let expected = capture(&gnu);
        assert_eq!(capture(&ours_path), expected, "program {index}");
        assert_eq!(expected.1, 42, "program {index} fixture");
    }
}
