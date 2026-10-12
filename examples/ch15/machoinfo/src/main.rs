//! machoinfo：读 Mach-O 的头和 load command，列出 segment、section 和依赖的 dylib。
//! 支持 fat（universal）文件：先读 fat header，再逐个架构解析。
//! 只用标准库；所有多字节字段按 Mach-O 规定的字节序读取。

use std::{env, fs, process};

const FAT_MAGIC: u32 = 0xcafe_babe; // fat_header 一律大端
const FAT_MAGIC_64: u32 = 0xcafe_babf;
const MH_MAGIC_64: u32 = 0xfeed_facf; // 本机小端读出来就是这个值

const LC_REQ_DYLD: u32 = 0x8000_0000;
const LC_SEGMENT_64: u32 = 0x19;
const LC_LOAD_DYLIB: u32 = 0xc;
const LC_ID_DYLIB: u32 = 0xd;
const LC_LOAD_WEAK_DYLIB: u32 = 0x18 | LC_REQ_DYLD;
const LC_REEXPORT_DYLIB: u32 = 0x1f | LC_REQ_DYLD;
const LC_RPATH: u32 = 0x1c | LC_REQ_DYLD;
const LC_MAIN: u32 = 0x28 | LC_REQ_DYLD;

type R<T> = Result<T, String>;

fn be32(b: &[u8], off: usize) -> R<u32> {
    let s = b.get(off..off + 4).ok_or_else(|| format!("truncated at {off:#x}"))?;
    Ok(u32::from_be_bytes(s.try_into().unwrap()))
}
fn be64(b: &[u8], off: usize) -> R<u64> {
    let s = b.get(off..off + 8).ok_or_else(|| format!("truncated at {off:#x}"))?;
    Ok(u64::from_be_bytes(s.try_into().unwrap()))
}
fn le32(b: &[u8], off: usize) -> R<u32> {
    let s = b.get(off..off + 4).ok_or_else(|| format!("truncated at {off:#x}"))?;
    Ok(u32::from_le_bytes(s.try_into().unwrap()))
}
fn le64(b: &[u8], off: usize) -> R<u64> {
    let s = b.get(off..off + 8).ok_or_else(|| format!("truncated at {off:#x}"))?;
    Ok(u64::from_le_bytes(s.try_into().unwrap()))
}

/// segname / sectname 是 16 字节、不一定以 NUL 结尾的字段
fn name16(b: &[u8], off: usize) -> R<String> {
    let s = b.get(off..off + 16).ok_or("truncated name")?;
    let end = s.iter().position(|&c| c == 0).unwrap_or(16);
    Ok(String::from_utf8_lossy(&s[..end]).into_owned())
}

/// load command 里 lc_str 指向的字符串：偏移相对于该命令起点，以 NUL 结尾
fn lc_str(cmd: &[u8], off: u32) -> R<String> {
    let s = cmd.get(off as usize..).ok_or("bad lc_str offset")?;
    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    Ok(String::from_utf8_lossy(&s[..end]).into_owned())
}

fn cpu_name(cputype: u32, sub: u32) -> String {
    match (cputype, sub & 0x00ff_ffff) {
        (0x0100_0007, _) => "x86_64".into(),
        (0x0100_000c, 2) => "arm64e".into(),
        (0x0100_000c, _) => "arm64".into(),
        (t, s) => format!("cpu {t:#x}/{s:#x}"),
    }
}

/// 解析一个 thin 的 64 位 Mach-O（可能是 fat 文件里的一片）
fn thin(b: &[u8]) -> R<()> {
    if le32(b, 0)? != MH_MAGIC_64 {
        return Err(format!("not a 64-bit little-endian Mach-O (magic {:#x})", le32(b, 0)?));
    }
    // struct mach_header_64：magic cputype cpusubtype filetype ncmds sizeofcmds flags reserved
    let (cputype, cpusub) = (le32(b, 4)?, le32(b, 8)?);
    let (filetype, ncmds, sizeofcmds, flags) = (le32(b, 12)?, le32(b, 16)?, le32(b, 20)?, le32(b, 24)?);
    println!(
        "arch {}  filetype {filetype}  ncmds {ncmds}  sizeofcmds {sizeofcmds}  flags {flags:#x}",
        cpu_name(cputype, cpusub)
    );
    let mut off = 32usize; // load command 紧跟在 32 字节的头后面
    let end = off + sizeofcmds as usize;
    if end > b.len() {
        return Err(format!("sizeofcmds {sizeofcmds} runs past end of file ({} bytes)", b.len()));
    }
    for _ in 0..ncmds {
        let (cmd, cmdsize) = (le32(b, off)?, le32(b, off + 4)? as usize);
        // cmdsize 来自文件，不可信：既不能越过 load command 区，也不能越过文件末尾
        if cmdsize < 8 || off + cmdsize > end || off + cmdsize > b.len() {
            return Err(format!("bad cmdsize {cmdsize} at {off:#x}"));
        }
        let c = b.get(off..off + cmdsize).ok_or("load command out of range")?;
        match cmd {
            LC_SEGMENT_64 => {
                // segment_command_64：cmd cmdsize segname[16] vmaddr vmsize fileoff filesize
                //                    maxprot initprot nsects flags，共 72 字节，后面跟 nsects 个 section_64
                let nsects = le32(c, 64)?;
                // 数字的写法和 otool -l 一致，便于对拍
                println!(
                    "segment {:<16} vmaddr {:#018x} vmsize {:#018x} fileoff {} filesize {} maxprot {:#010x} initprot {:#010x} nsects {nsects}",
                    name16(c, 8)?, le64(c, 24)?, le64(c, 32)?, le64(c, 40)?, le64(c, 48)?,
                    le32(c, 56)?, le32(c, 60)?
                );
                for i in 0..nsects as usize {
                    let s = 72 + i * 80; // section_64 每个 80 字节
                    println!(
                        "  section {:<16},{:<16} addr {:#018x} size {:#x}",
                        name16(c, s + 16)?, name16(c, s)?, le64(c, s + 32)?, le64(c, s + 40)?
                    );
                }
            }
            LC_LOAD_DYLIB | LC_LOAD_WEAK_DYLIB | LC_REEXPORT_DYLIB | LC_ID_DYLIB => {
                // dylib_command：cmd cmdsize { name(lc_str) timestamp current_version compat_version }
                let kind = match cmd {
                    LC_LOAD_DYLIB => "load",
                    LC_LOAD_WEAK_DYLIB => "weak",
                    LC_REEXPORT_DYLIB => "reexport",
                    _ => "id",
                };
                println!("dylib {kind:<8} {}", lc_str(c, le32(c, 8)?)?);
            }
            LC_RPATH => println!("rpath {}", lc_str(c, le32(c, 8)?)?),
            LC_MAIN => println!("main entryoff {:#x}", le64(c, 8)?),
            _ => {}
        }
        off += cmdsize;
    }
    Ok(())
}

fn run(path: &str) -> R<()> {
    let b = fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    match be32(&b, 0)? {
        m @ (FAT_MAGIC | FAT_MAGIC_64) => {
            // fat_header：magic nfat_arch；之后是 fat_arch（20 字节）或 fat_arch_64（32 字节）
            let n = be32(&b, 4)? as usize;
            let is64 = m == FAT_MAGIC_64;
            for i in 0..n {
                let a = 8 + i * if is64 { 32 } else { 20 };
                let (offset, size) = if is64 {
                    (be64(&b, a + 8)? as usize, be64(&b, a + 16)? as usize)
                } else {
                    (be32(&b, a + 8)? as usize, be32(&b, a + 12)? as usize)
                };
                println!("## fat slice {i}: offset {offset:#x} size {size:#x}");
                let slice = b.get(offset..offset + size).ok_or("fat slice out of range")?;
                thin(slice)?;
            }
            Ok(())
        }
        _ => thin(&b),
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: machoinfo <mach-o file>...");
        process::exit(2);
    }
    for p in &args {
        if let Err(e) = run(p) {
            eprintln!("machoinfo: {e}");
            process::exit(1);
        }
    }
}
