//! Supplied command-line I/O; all linking is performed by the student library.
use std::{env, fs, path::PathBuf, process::ExitCode};
fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let mut output = None;
    let mut inputs = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "-o" {
            if output.is_some() {
                return Err("duplicate -o".into());
            }
            output = Some(PathBuf::from(args.next().ok_or("-o requires a path")?));
        } else if arg.to_string_lossy().starts_with('-') {
            return Err("unsupported option".into());
        } else {
            inputs.push(PathBuf::from(arg));
        }
    }
    let output = output.ok_or("usage: class5 -o output input.o ...")?;
    if inputs.is_empty() {
        return Err("at least one input is required".into());
    }
    let objects = inputs
        .iter()
        .map(|p| fs::read(p).map_err(|e| format!("{}: {e}", p.display())))
        .collect::<Result<Vec<_>, _>>()?;
    let slices: Vec<_> = objects.iter().map(Vec::as_slice).collect();
    let image = class5::relocate::link(&slices, b"_start").map_err(|e| format!("{e:?}"))?;
    fs::write(&output, image).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(output, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("class5: {e}");
            ExitCode::FAILURE
        }
    }
}
