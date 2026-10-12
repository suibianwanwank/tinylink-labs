//! A linker driver that `rustc` can invoke.
//!
//! This driver lets `rustc` run a linker executable with
//! a GNU-ld-style command line. This class turns the cumulative icf
//! static-PIE backend into such an executable: it expands response files, parses the
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
/// programs. Reuse icf ICF, tls PIE/GOT/TLS/merge and debug metadata.
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
    let _ = args;
    todo!("C14.1: interpret a GNU-ld-style command line")
}

/// Find the file for one library: search `dirs` in order for `libNAME.a`, or
/// for `NAME` itself when verbatim. `exists` reports whether a path is a file.
pub fn find_library(
    name: &str,
    verbatim: bool,
    dirs: &[PathBuf],
    exists: &dyn Fn(&Path) -> bool,
) -> Result<PathBuf, Error> {
    let _ = (name, verbatim, dirs, exists);
    todo!("C14.2: search library directories like a linker")
}

/// Expand GNU-style response files. An argument `@PATH` whose file `read`
/// can supply is replaced by the arguments in it, recursively; one it cannot
/// read stays as written. README defines quoting and the nesting limit.
pub fn expand(
    args: &[OsString],
    read: &dyn Fn(&Path) -> Option<Vec<u8>>,
) -> Result<Vec<OsString>, Error> {
    let _ = (args, read);
    todo!("C14.3: expand @file arguments with GNU quoting rules")
}

/// Expand response files, parse, resolve libraries, read inputs, link with
/// the cumulative [`link`] interface, and write an executable file.
pub fn run(args: &[OsString]) -> Result<(), Error> {
    let _ = args;
    todo!("C14.4: drive the static-PIE backend from a command line")
}
