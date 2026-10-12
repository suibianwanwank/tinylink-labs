//! tinylink 的命令行入口。
//!
//! 退出码约定（后面的里程碑沿用）：
//! - 0：成功，可能带警告
//! - 1：链接出错，或者输入文件格式有错
//! - 2：命令行用法错误、读不到输入文件、写不出输出

use std::fmt::Display;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::process::ExitCode;

use tinylink::elf::ElfFile;
use tinylink::input::{InputError, InputName};
use tinylink::nm::NmView;
use tinylink::readelf::{HeaderView, RelocsView, SectionsView, SegmentsView, SymbolsView};

const USAGE: &str = "\
usage: tinylink toy <input.tl>
       tinylink readelf (-h | -l | -S | -s | -r) <file>
       tinylink nm <file>
       tinylink link -o <output> <input.o>
       tinylink --version";

/// 把结果写到 stdout。用 `write!` 而不用 `print!`：
/// 后者写失败时会 panic，前者把错误作为 `Result` 交给调用者处理。
fn write_stdout(value: &dyn Display) -> io::Result<()> {
    let mut out = io::stdout().lock();
    write!(out, "{value}")?;
    out.flush()
}

/// 写 stdout，并把写失败翻译成退出码。读 stdout 的一方提前关了管道
/// （比如 `| head -1`），剩下的输出没人要，不算错误。
fn emit(value: &dyn Display) -> Result<(), ExitCode> {
    match write_stdout(value) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => {
            eprintln!("tinylink: error: cannot write output: {e}");
            Err(ExitCode::from(2))
        }
    }
}

fn read_input(path: &str) -> Result<Vec<u8>, ExitCode> {
    std::fs::read(path).map_err(|e| {
        eprintln!("tinylink: error: cannot read '{path}': {e}");
        ExitCode::from(2)
    })
}

/// 报告一个输入文件里的错误，退出码 1。错误本身只说文件里哪里不对，文件是谁由 `InputError` 加上。
fn input_error<E: Display>(path: &str, error: E) -> ExitCode {
    let error = InputError::new(InputName::Path(path.to_owned()), error);
    eprintln!("tinylink: error: {error}");
    ExitCode::from(1)
}

fn toy(path: &str) -> Result<ExitCode, ExitCode> {
    let bytes = read_input(path)?;
    let src = match tinylink::toy::decode(&bytes) {
        Ok(src) => src,
        Err(diag) => {
            eprintln!("{}", diag.render(path));
            return Ok(ExitCode::from(1));
        }
    };

    let outcome = tinylink::toy::link(src);
    if let Some(image) = &outcome.image {
        emit(image)?;
    }
    for diag in &outcome.diagnostics {
        eprintln!("{}", diag.render(path));
    }
    Ok(if outcome.has_errors() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

/// `readelf` 能显示的五种视图，对应 `-h`、`-l`、`-S`、`-s`、`-r`。
#[derive(Clone, Copy)]
enum ReadelfView {
    Header,
    Segments,
    Sections,
    Symbols,
    Relocs,
}

fn readelf(which: ReadelfView, path: &str) -> Result<ExitCode, ExitCode> {
    let bytes = read_input(path)?;
    let file = ElfFile::parse(&bytes).map_err(|e| input_error(path, e))?;
    // 先做完所有可能失败的解析，再开始输出：出错时 stdout 上不会留下半张表。
    let view: Box<dyn Display> = match which {
        ReadelfView::Header => Box::new(HeaderView(&file)),
        ReadelfView::Segments => Box::new(SegmentsView(&file)),
        ReadelfView::Sections => Box::new(SectionsView(&file)),
        ReadelfView::Symbols => {
            Box::new(SymbolsView::new(&file).map_err(|e| input_error(path, e))?)
        }
        ReadelfView::Relocs => Box::new(RelocsView::new(&file).map_err(|e| input_error(path, e))?),
    };
    emit(&view)?;
    Ok(ExitCode::SUCCESS)
}

fn nm(path: &str) -> Result<ExitCode, ExitCode> {
    let bytes = read_input(path)?;
    let file = ElfFile::parse(&bytes).map_err(|e| input_error(path, e))?;
    let view = NmView::new(&file).map_err(|e| input_error(path, e))?;
    if view.is_empty() {
        eprintln!("tinylink: nm: {path}: no symbols");
    }
    emit(&view)?;
    Ok(ExitCode::SUCCESS)
}

fn link(output: &str, input: &str) -> Result<ExitCode, ExitCode> {
    let bytes = read_input(input)?;
    let file = ElfFile::parse(&bytes).map_err(|e| input_error(input, e))?;
    let image = tinylink::link::link(&file).map_err(|e| input_error(input, e))?;
    // 链接成功之后才创建输出文件，出错时不会留下一个残缺的可执行文件。
    let written = std::fs::write(output, &image)
        .and_then(|()| std::fs::set_permissions(output, std::fs::Permissions::from_mode(0o755)));
    if let Err(e) = written {
        eprintln!("tinylink: error: cannot write '{output}': {e}");
        return Err(ExitCode::from(2));
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    // std::env::args 遇到不是 UTF-8 的参数会 panic；args_os 不会，由这里报一个用法错误。
    let mut args = Vec::new();
    for arg in std::env::args_os().skip(1) {
        match arg.into_string() {
            Ok(arg) => args.push(arg),
            Err(arg) => {
                eprintln!("tinylink: error: argument is not valid UTF-8: {arg:?}");
                return ExitCode::from(2);
            }
        }
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let result = match args.as_slice() {
        ["--version"] => emit(&format_args!("tinylink {}\n", env!("CARGO_PKG_VERSION")))
            .map(|()| ExitCode::SUCCESS),
        ["toy", path] => toy(path),
        ["readelf", "-h", path] => readelf(ReadelfView::Header, path),
        ["readelf", "-l", path] => readelf(ReadelfView::Segments, path),
        ["readelf", "-S", path] => readelf(ReadelfView::Sections, path),
        ["readelf", "-s", path] => readelf(ReadelfView::Symbols, path),
        ["readelf", "-r", path] => readelf(ReadelfView::Relocs, path),
        ["nm", path] => nm(path),
        ["link", "-o", output, input] => link(output, input),
        _ => {
            eprintln!("{USAGE}");
            Ok(ExitCode::from(2))
        }
    };
    // Ok 和 Err 里装的都是退出码：Err 只是让 `?` 能提前返回。
    result.unwrap_or_else(|code| code)
}
