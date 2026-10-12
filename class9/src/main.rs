//! Supplied command-line I/O; all linking is performed by the student library.
use std::{env, fs, path::PathBuf, process::ExitCode};
fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let mut output = None;
    let mut inputs = Vec::new();
    let mut map_path = None;
    let mut keep = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "-o" {
            if output.is_some() {
                return Err("duplicate -o".into());
            }
            output = Some(PathBuf::from(args.next().ok_or("-o requires a path")?));
        } else if arg == "-Map" {
            if map_path.is_some() {
                return Err("duplicate -Map".into());
            }
            map_path = Some(PathBuf::from(args.next().ok_or("-Map requires a path")?));
        } else if arg == "--keep" {
            let name = args.next().ok_or("--keep requires a symbol")?;
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStrExt;
                keep.push(name.as_bytes().to_vec());
            }
            #[cfg(not(unix))]
            {
                keep.push(name.to_string_lossy().as_bytes().to_vec());
            }
        } else if arg.to_string_lossy().starts_with('-') {
            return Err("unsupported option".into());
        } else {
            inputs.push(PathBuf::from(arg));
        }
    }
    let output = output.ok_or("usage: class9 -o output [-Map map] [--keep symbol] input.o ...")?;
    if inputs.is_empty() {
        return Err("at least one input is required".into());
    }
    let objects = inputs
        .iter()
        .map(|p| fs::read(p).map_err(|e| format!("{}: {e}", p.display())))
        .collect::<Result<Vec<_>, _>>()?;
    let slices: Vec<_> = objects
        .iter()
        .map(|b| {
            if b.starts_with(b"!<arch>\n") || b.starts_with(b"!<thin>\n") {
                class9::archive::Input::Archive(b)
            } else {
                class9::archive::Input::Object(b)
            }
        })
        .collect();
    if map_path.as_ref() == Some(&output) {
        return Err("map and executable paths must differ".into());
    }
    let keep: Vec<_> = keep.iter().map(Vec::as_slice).collect();
    let image = class9::pie::link(&slices, b"_start", &keep).map_err(|e| format!("{e:?}"))?;
    if let Some(path) = map_path {
        fs::write(path, &image.map).map_err(|e| e.to_string())?;
    }
    fs::write(&output, image.bytes).map_err(|e| e.to_string())?;
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
            eprintln!("class9: {e}");
            ExitCode::FAILURE
        }
    }
}
