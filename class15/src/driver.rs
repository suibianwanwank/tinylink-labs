//! A linker driver that `rustc` can invoke.
//!
//! This driver exposes the linker through the GNU-style command line used by
//! rustc on the supported Linux target. This class turns the cumulative class13 static-PIE
//! backend into such an executable: it expands response files, parses the
//! options `rustc` passes, searches library directories, and chooses a section
//! policy for code compiled by LLVM. README is the contract.
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::gc::Role;

/// One input in command-line order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputSpec {
    /// A file named directly: an object or an archive.
    Path(PathBuf),
    /// `-lNAME` (searched as `libNAME.a`) or, when `verbatim`, `-l:NAME`.
    Library { name: String, verbatim: bool },
}

/// A parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub output: PathBuf,
    /// Entry symbol; `_start` unless `-e` or `--entry` says otherwise.
    pub entry: Vec<u8>,
    pub inputs: Vec<InputSpec>,
    /// `-L` directories in command-line order.
    pub search: Vec<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// An option this driver does not implement, verbatim.
    UnsupportedOption(String),
    /// An option is missing its argument.
    MissingArgument(String),
    /// No `-o`, or more than one.
    InvalidOutput,
    /// No input files or libraries.
    NoInputs,
    /// `-lNAME` was not found in any search directory.
    LibraryNotFound(String),
    /// Response files nested more than 16 deep.
    ResponseFileDepth,
    Io(String),
    Link(crate::icf::Error),
}
impl From<crate::icf::Error> for Error {
    fn from(e: crate::icf::Error) -> Self {
        Self::Link(e)
    }
}

/// Section policy for LLVM output. Supplied.
///
/// The `SHF_GROUP` flag (COMDAT membership) is ignored; `.eh_frame` is an
/// annotation handled by the explicitly selected abort-only policy. Mergeable
/// inputs retain their split role and piece mapping. Everything else follows
/// `crate::tls::policy` after masking group membership.
pub fn policy(section: &crate::sections::Section, name: &[u8]) -> Option<Role> {
    const SHF_GROUP: u64 = 0x200;
    const SHT_X86_64_UNWIND: u32 = 0x7000_0001;
    let flags = section.flags & !SHF_GROUP;
    match (section.kind, flags) {
        (1 | SHT_X86_64_UNWIND, 2) if name == b".eh_frame" => Some(Role::Annotation),
        _ => {
            let masked = crate::sections::Section {
                flags,
                ..clone_section(section)
            };
            crate::tls::policy(&masked, name)
        }
    }
}

/// Supplied cumulative backend for this driver's freestanding abort-only
/// programs. Reuse class13 ICF, class11 PIE/GOT/TLS/merge and class12 metadata.
/// Runtime exception unwinding and backtraces are outside this driver contract.
pub fn link(
    inputs: &[crate::archive::Input<'_>],
    entry: &[u8],
) -> Result<crate::gc::Linked, Error> {
    Ok(crate::icf::link_with(
        inputs,
        entry,
        &[],
        &[],
        policy,
        crate::icf::Backend::StaticPie(crate::tls::UnwindPolicy::AbortOnly),
    )?)
}

fn clone_section(s: &crate::sections::Section) -> crate::sections::Section {
    crate::sections::Section {
        name_offset: s.name_offset,
        kind: s.kind,
        flags: s.flags,
        offset: s.offset,
        size: s.size,
        link: s.link,
        info: s.info,
        alignment: s.alignment,
        entry_size: s.entry_size,
    }
}

/// Parse the arguments after the program name. README lists every accepted
/// option and how it maps onto this linker.
pub fn parse(args: &[OsString]) -> Result<Invocation, Error> {
    let mut output = None;
    let mut entry = b"_start".to_vec();
    let mut inputs = Vec::new();
    let mut search = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let Some(text) = arg.to_str() else {
            inputs.push(InputSpec::Path(PathBuf::from(arg)));
            continue;
        };
        let mut value = |option: &str| -> Result<OsString, Error> {
            args.next()
                .cloned()
                .ok_or_else(|| Error::MissingArgument(option.to_string()))
        };
        let library = |name: &str| match name.strip_prefix(':') {
            Some(file) => InputSpec::Library {
                name: file.to_string(),
                verbatim: true,
            },
            None => InputSpec::Library {
                name: name.to_string(),
                verbatim: false,
            },
        };
        match text {
            "-o" => {
                if output.replace(PathBuf::from(value(text)?)).is_some() {
                    return Err(Error::InvalidOutput);
                }
            }
            "-e" | "--entry" => entry = value(text)?.into_encoded_bytes(),
            "-L" => search.push(PathBuf::from(value(text)?)),
            "-l" => {
                let name = value(text)?;
                let name = name.to_str().ok_or_else(|| {
                    Error::UnsupportedOption(format!("-l {}", name.to_string_lossy()))
                })?;
                inputs.push(library(name));
            }
            "-z" => {
                let keyword = value(text)?;
                if !matches!(keyword.to_str(), Some("noexecstack" | "relro" | "now")) {
                    return Err(Error::UnsupportedOption(format!(
                        "-z {}",
                        keyword.to_string_lossy()
                    )));
                }
            }
            "-pie" | "--pic-executable" | "--gc-sections" | "--as-needed" | "--no-as-needed"
            | "-Bstatic" | "-Bdynamic" | "-static" | "--eh-frame-hdr" | "-O0" | "-O1" | "-O2"
            | "-O3" => {}
            _ if text.starts_with("--output=") => {
                if output
                    .replace(PathBuf::from(&text["--output=".len()..]))
                    .is_some()
                {
                    return Err(Error::InvalidOutput);
                }
            }
            _ if text.starts_with("--entry=") => {
                entry = text.as_bytes()["--entry=".len()..].to_vec()
            }
            _ if text.starts_with("--library-path=") => {
                search.push(PathBuf::from(&text["--library-path=".len()..]))
            }
            _ if text.starts_with("--library=") => {
                inputs.push(library(&text["--library=".len()..]))
            }
            _ if text.starts_with("-L") && text.len() > 2 => search.push(PathBuf::from(&text[2..])),
            _ if text.starts_with("-l") && text.len() > 2 => inputs.push(library(&text[2..])),
            _ if text.starts_with('-') => return Err(Error::UnsupportedOption(text.to_string())),
            _ => inputs.push(InputSpec::Path(PathBuf::from(arg))),
        }
    }
    let output = output.ok_or(Error::InvalidOutput)?;
    if inputs.is_empty() {
        return Err(Error::NoInputs);
    }
    Ok(Invocation {
        output,
        entry,
        inputs,
        search,
    })
}

/// Find the file for one library: search `dirs` in order for `libNAME.a`, or
/// for `NAME` itself when verbatim. `exists` reports whether a path is a file.
pub fn find_library(
    name: &str,
    verbatim: bool,
    dirs: &[PathBuf],
    exists: &dyn Fn(&Path) -> bool,
) -> Result<PathBuf, Error> {
    let file = if verbatim {
        name.to_string()
    } else {
        format!("lib{name}.a")
    };
    dirs.iter()
        .map(|dir| dir.join(&file))
        .find(|path| exists(path))
        .ok_or_else(|| Error::LibraryNotFound(name.to_string()))
}

/// Expand GNU-style response files. An argument `@PATH` whose file `read`
/// can supply is replaced by the arguments in it, recursively; one it cannot
/// read stays as written. README defines quoting and the nesting limit.
pub fn expand(
    args: &[OsString],
    read: &dyn Fn(&Path) -> Option<Vec<u8>>,
) -> Result<Vec<OsString>, Error> {
    use std::ffi::OsStr;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    fn split(bytes: &[u8]) -> Vec<OsString> {
        let mut out = Vec::new();
        let mut current = Vec::new();
        let (mut started, mut single, mut double, mut escaped) = (false, false, false, false);
        for &b in bytes {
            if escaped {
                current.push(b);
                escaped = false;
                continue;
            }
            match b {
                b'\\' => (escaped, started) = (true, true),
                b'\'' if !double => (single, started) = (!single, true),
                b'"' if !single => (double, started) = (!double, true),
                b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c if !single && !double => {
                    if started {
                        out.push(OsString::from_vec(std::mem::take(&mut current)));
                        started = false;
                    }
                }
                _ => {
                    current.push(b);
                    started = true;
                }
            }
        }
        if started {
            out.push(OsString::from_vec(current));
        }
        out
    }
    fn go(
        args: &[OsString],
        read: &dyn Fn(&Path) -> Option<Vec<u8>>,
        depth: usize,
        out: &mut Vec<OsString>,
    ) -> Result<(), Error> {
        for arg in args {
            let contents = arg
                .as_bytes()
                .strip_prefix(b"@")
                .filter(|p| !p.is_empty())
                .and_then(|p| read(Path::new(OsStr::from_bytes(p))));
            match contents {
                Some(contents) => {
                    if depth == 16 {
                        return Err(Error::ResponseFileDepth);
                    }
                    go(&split(&contents), read, depth + 1, out)?;
                }
                None => out.push(arg.clone()),
            }
        }
        Ok(())
    }
    let mut out = Vec::with_capacity(args.len());
    go(args, read, 0, &mut out)?;
    Ok(out)
}

/// Expand response files, parse, resolve libraries, read inputs, link with
/// the cumulative [`link`] interface, and write an executable file.
pub fn run(args: &[OsString]) -> Result<(), Error> {
    let args = expand(args, &|path| std::fs::read(path).ok())?;
    let invocation = parse(&args)?;
    let read = |path: &Path| {
        std::fs::read(path).map_err(|e| Error::Io(format!("{}: {e}", path.display())))
    };
    let mut files = Vec::with_capacity(invocation.inputs.len());
    for spec in &invocation.inputs {
        let path = match spec {
            InputSpec::Path(path) => path.clone(),
            InputSpec::Library { name, verbatim } => {
                find_library(name, *verbatim, &invocation.search, &|p| p.is_file())?
            }
        };
        let bytes = read(&path)?;
        files.push((bytes.starts_with(b"!<arch>\n"), bytes));
    }
    let inputs: Vec<_> = files
        .iter()
        .map(|(archive, bytes)| {
            if *archive {
                crate::archive::Input::Archive(bytes)
            } else {
                crate::archive::Input::Object(bytes)
            }
        })
        .collect();
    let linked = link(&inputs, &invocation.entry)?;
    let write = |path: &Path, bytes: &[u8]| {
        std::fs::write(path, bytes).map_err(|e| Error::Io(format!("{}: {e}", path.display())))
    };
    write(&invocation.output, &linked.bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&invocation.output, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| Error::Io(format!("{}: {e}", invocation.output.display())))?;
    }
    Ok(())
}
