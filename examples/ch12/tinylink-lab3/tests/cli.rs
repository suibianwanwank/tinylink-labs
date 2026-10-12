//! 命令行层面的行为：这些情形没法写成 `.tl` 用例。

use std::path::Path;
use std::process::{Command, Stdio};

fn golden(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/toy/golden")
        .join(name)
        .display()
        .to_string()
}

/// stdout 的读端已经关闭（相当于 `tinylink ... | head -0`）时，
/// tinylink 不能 panic，退出码仍按链接结果决定。
#[test]
fn closed_stdout_is_not_a_crash() {
    let (reader, writer) = std::io::pipe().expect("create pipe");
    // 先关掉读端再启动子进程，子进程第一次写 stdout 就一定会遇到 EPIPE。
    drop(reader);
    let output = Command::new(env!("CARGO_BIN_EXE_tinylink"))
        .args(["toy", &golden("fill-memory.tl")])
        .stdout(writer)
        .stderr(Stdio::piped())
        .output()
        .expect("run tinylink");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
}

#[test]
fn missing_input_file_exits_2() {
    let output = Command::new(env!("CARGO_BIN_EXE_tinylink"))
        .args(["toy", "no-such-file.tl"])
        .output()
        .expect("run tinylink");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("tinylink: error: cannot read 'no-such-file.tl'"),
        "{stderr}"
    );
}

#[test]
fn version_with_closed_stdout_is_not_a_crash() {
    let (reader, writer) = std::io::pipe().expect("create pipe");
    drop(reader);
    let output = Command::new(env!("CARGO_BIN_EXE_tinylink"))
        .arg("--version")
        .stdout(writer)
        .stderr(Stdio::piped())
        .output()
        .expect("run tinylink");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
    assert_eq!(output.status.code(), Some(0), "stderr: {stderr}");
}

#[test]
fn non_utf8_argument_exits_2() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let output = Command::new(env!("CARGO_BIN_EXE_tinylink"))
        .args([OsStr::new("nm"), OsStr::from_bytes(b"caf\xe9.o")])
        .output()
        .expect("run tinylink");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("tinylink: error: argument is not valid UTF-8"),
        "{stderr}"
    );
}
