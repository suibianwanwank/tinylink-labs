//! Supplied I/O driver. Linker algorithms belong in lib.rs.
use std::{env, fs, path::PathBuf, process::ExitCode};
fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let input = PathBuf::from(args.next().ok_or("usage: class4 input.o output")?);
    let output = PathBuf::from(args.next().ok_or("usage: class4 input.o output")?);
    if args.next().is_some() {
        return Err("usage: class4 input.o output".into());
    }
    let source = fs::read(&input).map_err(|e| format!("{}: {e}", input.display()))?;
    let image =
        class4::executable::link_single(&source, b"_start").map_err(|e| format!("{e:?}"))?;
    fs::write(&output, image).map_err(|e| format!("{}: {e}", output.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&output, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("class4: {e}");
            ExitCode::FAILURE
        }
    }
}
