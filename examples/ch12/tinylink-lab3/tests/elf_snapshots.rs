//! 拿 tinylink 的 readelf 和 nm 与 GNU binutils 2.45 的输出逐字节对拍。
//!
//! 参考输出由 `tests/elf/fixtures/gen.sh` 生成：`x86_64-linux-musl-readelf -W` 和
//! `LC_ALL=C x86_64-linux-musl-nm`。对这里的全部目标文件，两者的输出完全一致。
//!
//! 除了目标文件，还有 ld.lld 和 GNU ld 链接出来的两个可执行文件（`*.elf`），用来对拍 `readelf -l`。
//!
//! 有意的差异（这些情形不在测试的文件里，遇到时两边输出会不同）：
//! - tinylink 一次只接受一个选项（`-h`、`-l`、`-S`、`-s`、`-r` 之一），GNU readelf 可以组合；
//! - 文件有任何结构错误时，tinylink 整个拒绝并报一个带位置的错误；GNU readelf 尽量打印，
//!   在出错的地方插入警告；
//! - 只认识 x86-64 的重定位类型名，其他架构的重定位类型显示为 `unrecognized: <编号>`；
//! - 不支持 `SHT_REL`、`.dynsym`、`SHN_XINDEX` 和扩展节编号，遇到 `SHN_XINDEX` 时报"不支持"；
//! - GNU nm 按当前 locale 排序，tinylink 总是按字节排序，相当于 `LC_ALL=C`；
//!   同名符号保持符号表里的顺序，GNU nm 对同名符号还会按值等次序再排，两者可能不同；
//! - nm 不认识 `STT_GNU_IFUNC`，这类符号按所在节给字母，GNU nm 显示 `i`；
//! - readelf -h 只认识 OS/ABI 0（System V）和 3（GNU）；GNU readelf 在 OSABI 为 GNU 时
//!   还会识别 `SHF_GNU_RETAIN` 等 OS 专用标志，tinylink 的 Flg 列把它们显示成 `o`；
//! - readelf -s 对没有节头表的文件（比如 M2 的产物）和 GNU 一样，打印一个空行和
//!   "Dynamic symbol information is not available for displaying symbols."；
//!   但 GNU 对只有 `.dynsym` 的文件会打印动态符号表，tinylink 不读 `.dynsym`；
//! - 没有节名字符串表（`e_shstrndx` 为 0）时，`-S` 和 `-r` 与 GNU 一致（节名显示成
//!   `<no-strings>`，重定位节显示 `sh_name` 的数值）；`-s` 的 Ndx 列 GNU 也显示成
//!   `<no-strings>`，tinylink 照常显示节号；GNU nm 认为这样的文件"string table index"损坏、
//!   不列符号，tinylink 照常列出；`-l` 和 GNU 一样不打印"节到段的映射"（`exit42-lld.nostr`）；
//! - 名字不是 UTF-8 时，tinylink 把坏字节显示成 U+FFFD，GNU 原样输出字节；
//! - readelf -l 的"节到段的映射"用简化的规则：只看可分配的节，不处理 TLS 节和
//!   `PT_DYNAMIC` 的特例；不打印 `PT_INTERP` 后面的 `[Requesting program interpreter: ...]`。

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/elf/fixtures")
}

fn tinylink(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_tinylink"))
        .args(args)
        .current_dir(fixtures())
        .output()
        .expect("run tinylink");
    assert!(
        output.status.success(),
        "tinylink {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf-8 output")
}

fn files_ending_with(ext: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(fixtures())
        .expect("read fixtures")
        .map(|e| {
            e.expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| n.ends_with(ext))
        .collect();
    names.sort();
    names
}

type View = (&'static [&'static str], &'static str);

/// 对每个文件运行每个视图，返回和 GNU 不一致的 (文件, 视图)。
fn compare(files: &[String], suffix: &str, views: &[View]) -> Vec<String> {
    let mut mismatches = Vec::new();
    for object in files {
        let base = object.trim_end_matches(suffix);
        for &(args, ext) in views {
            let mut full = args.to_vec();
            full.push(object);
            let actual = tinylink(&full);
            let expected_path = fixtures().join("expected").join(format!("{base}.{ext}"));
            let expected = std::fs::read_to_string(&expected_path).expect("read expected");
            if actual != expected {
                mismatches.push(format!("{object} {ext}"));
            }
        }
    }
    mismatches
}

#[test]
fn readelf_and_nm_match_gnu_binutils() {
    let views: &[View] = &[
        (&["readelf", "-h"], "header"),
        (&["readelf", "-l"], "segments"),
        (&["readelf", "-S"], "sections"),
        (&["readelf", "-s"], "symbols"),
        (&["readelf", "-r"], "relocs"),
        (&["nm"], "nm"),
    ];
    let objects = files_ending_with(".o");
    assert!(objects.len() >= 10, "fixtures missing: {objects:?}");
    let mismatches = compare(&objects, ".o", views);
    assert!(mismatches.is_empty(), "differs from GNU: {mismatches:?}");
}

#[test]
fn executables_match_gnu_binutils() {
    let views: &[View] = &[
        (&["readelf", "-h"], "header"),
        (&["readelf", "-l"], "segments"),
        (&["readelf", "-S"], "sections"),
        (&["readelf", "-s"], "symbols"),
        (&["nm"], "nm"),
    ];
    let executables = files_ending_with(".elf");
    assert_eq!(executables.len(), 2, "fixtures missing: {executables:?}");
    let mismatches = compare(&executables, ".elf", views);
    assert!(mismatches.is_empty(), "differs from GNU: {mismatches:?}");
}

#[test]
fn linked_executables_match_snapshots() {
    let dir = std::env::temp_dir().join(format!("tinylink-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    for base in ["exit42", "exit42-a64"] {
        let out = dir.join(base);
        let out_str = out.to_str().expect("utf-8 path");
        tinylink(&["link", "-o", out_str, &format!("{base}.o")]);

        // 字节级快照：布局的任何变化都会让这里失败，确认无误后用 gen.sh 重新生成。
        let actual = std::fs::read(&out).expect("read output");
        let golden =
            std::fs::read(fixtures().join(format!("expected/{base}.exe"))).expect("golden");
        assert_eq!(actual, golden, "{base}: output differs from golden file");

        // tinylink 自己的 readelf -h 读出来的，也要和 GNU readelf 一致。
        let header = tinylink(&["readelf", "-h", out_str]);
        let expected =
            std::fs::read_to_string(fixtures().join(format!("expected/{base}.exe.header")))
                .unwrap();
        assert_eq!(header, expected, "{base}: readelf -h differs from GNU");
        let segments = tinylink(&["readelf", "-l", out_str]);
        let expected =
            std::fs::read_to_string(fixtures().join(format!("expected/{base}.exe.segments")))
                .unwrap();
        assert_eq!(segments, expected, "{base}: readelf -l differs from GNU");
        let symbols = tinylink(&["readelf", "-s", out_str]);
        let expected =
            std::fs::read_to_string(fixtures().join(format!("expected/{base}.exe.symbols")))
                .unwrap();
        assert_eq!(symbols, expected, "{base}: readelf -s differs from GNU");
    }
    std::fs::remove_dir_all(&dir).expect("remove temp dir");
}

/// ld.lld 的产物把 `e_shstrndx` 改成 0：没有节名字符串表。
#[test]
fn no_section_names_match_gnu() {
    for (args, ext) in [
        (&["readelf", "-l"], "segments"),
        (&["readelf", "-S"], "sections"),
    ] {
        let mut full = args.to_vec();
        full.push("exit42-lld.nostr");
        let actual = tinylink(&full);
        let expected =
            std::fs::read_to_string(fixtures().join(format!("expected/exit42-lld.nostr.{ext}")))
                .expect("read expected");
        assert_eq!(actual, expected, "exit42-lld.nostr {ext} differs from GNU");
    }
}
