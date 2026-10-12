//! Assemble the supplied startup code with the native Linux toolchain.
use std::{env, path::Path, process::Command};

fn run(command: &mut Command) {
    let output = command.output().unwrap_or_else(|error| {
        panic!("cannot run {command:?}: {error}");
    });
    assert!(
        output.status.success(),
        "{command:?} failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assemble(out: &Path, name: &str) {
    let source = format!("src/runtime/{name}.S");
    println!("cargo:rerun-if-changed={source}");
    let object = out.join(format!("{name}.o"));
    run(Command::new("as")
        .arg("--64")
        .arg(source)
        .arg("-o")
        .arg(&object));
    run(Command::new("objcopy")
        .args(["-O", "binary", "--only-section=.text"])
        .arg(object)
        .arg(out.join(format!("{name}.bin"))));
}

fn main() {
    let host = env::var("HOST").expect("Cargo must set HOST");
    let target = env::var("TARGET").expect("Cargo must set TARGET");
    assert!(
        host == target && target.starts_with("x86_64-unknown-linux-"),
        "this snapshot requires native x86-64 Linux"
    );
    let out = env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR");
    let out = Path::new(&out);
    assemble(out, "startup");
}
