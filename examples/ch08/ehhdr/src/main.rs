// ehhdr：读 ELF64 小端文件的 .eh_frame_hdr。
//   ehhdr 文件          打印整张表
//   ehhdr 文件 PC       二分查找 PC 所在的 FDE
//   ehhdr 文件 --check  检查链接器生成的表是否自洽
use std::{env, fs, process::exit};

fn u16_at(b: &[u8], o: usize) -> u16 { u16::from_le_bytes(b[o..o + 2].try_into().unwrap()) }
fn u32_at(b: &[u8], o: usize) -> u32 { u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) }
fn i32_at(b: &[u8], o: usize) -> i64 { u32_at(b, o) as i32 as i64 }
fn u64_at(b: &[u8], o: usize) -> u64 { u64::from_le_bytes(b[o..o + 8].try_into().unwrap()) }
fn die(msg: &str) -> ! { eprintln!("ehhdr: {msg}"); exit(1) }

const PT_LOAD: u32 = 1;
const PT_GNU_EH_FRAME: u32 = 0x6474_e550;

struct Seg { ty: u32, off: u64, vaddr: u64, filesz: u64 }

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 { die("用法：ehhdr 文件 [PC | --check]") }
    let b = fs::read(&args[1]).unwrap_or_else(|e| die(&format!("读 {} 失败：{e}", args[1])));
    if b.len() < 64 || &b[..4] != b"\x7fELF" || b[4] != 2 || b[5] != 1 { die("只支持 ELF64 小端") }

    // 程序头表：e_phoff 在 0x20，e_phentsize 在 0x36，e_phnum 在 0x38
    let (phoff, phentsize, phnum) = (u64_at(&b, 0x20) as usize, u16_at(&b, 0x36) as usize, u16_at(&b, 0x38) as usize);
    let segs: Vec<Seg> = (0..phnum).map(|i| {
        let p = phoff + i * phentsize;
        Seg { ty: u32_at(&b, p), off: u64_at(&b, p + 8), vaddr: u64_at(&b, p + 16), filesz: u64_at(&b, p + 32) }
    }).collect();
    // 虚拟地址 -> (文件偏移, 所在 PT_LOAD 在文件里的结束偏移)
    let v2o = |va: u64| -> Option<(usize, usize)> {
        segs.iter().find(|s| s.ty == PT_LOAD && va >= s.vaddr && va < s.vaddr + s.filesz)
            .map(|s| ((va - s.vaddr + s.off) as usize, (s.off + s.filesz) as usize))
    };

    let hdr = segs.iter().find(|s| s.ty == PT_GNU_EH_FRAME).unwrap_or_else(|| die("没有 PT_GNU_EH_FRAME"));
    let (h, hva) = (hdr.off as usize, hdr.vaddr);
    if (b[h], b[h + 1], b[h + 2], b[h + 3]) != (1, 0x1b, 0x03, 0x3b) { die("只处理 version 1、编码 1b/03/3b") }
    let eh_frame = (hva + 4).wrapping_add(i32_at(&b, h + 4) as u64); // pcrel：相对字段自身
    let n = u32_at(&b, h + 8) as usize;

    // 表项：(initial_location, fde)，都是 datarel sdata4，相对 .eh_frame_hdr 起点
    let entry = |i: usize| -> (u64, u64) {
        let o = h + 12 + i * 8;
        (hva.wrapping_add(i32_at(&b, o) as u64), hva.wrapping_add(i32_at(&b, o + 4) as u64))
    };
    // 读一条 FDE：(CIE 地址, pc_begin, pc_range)；本程序假定 CIE 的 R 编码为 0x1b
    let read_fde = |fde: u64| -> (u64, u64, u64) {
        let (f, _) = v2o(fde).unwrap_or_else(|| die(&format!("FDE {fde:#x} 不在任何 PT_LOAD 里")));
        let cie = fde + 4 - u32_at(&b, f + 4) as u64;
        (cie, (fde + 8).wrapping_add(i32_at(&b, f + 8) as u64), u32_at(&b, f + 12) as u64)
    };

    println!(".eh_frame_hdr @ {hva:#x}, .eh_frame @ {eh_frame:#x}, fde_count = {n}");
    match args.get(2).map(String::as_str) {
        None => for i in 0..n { let (loc, fde) = entry(i); println!("  [{i}] {loc:#x} -> FDE @ {fde:#x}") },
        Some("--check") => {
            // 顺着 .eh_frame 走一遍，记下每条记录的起点和种类（CIE 的 id 为 0）
            let (start, end) = v2o(eh_frame).unwrap_or_else(|| die(".eh_frame 不在任何 PT_LOAD 里"));
            let (mut o, mut fdes, mut cies) = (start, Vec::new(), Vec::new());
            while o + 8 <= end {
                let len = u32_at(&b, o) as usize;
                if len == 0 { break }               // 终止记录
                if len == 0xffff_ffff { die("不处理 64 位长度的记录") }
                let at = eh_frame + (o - start) as u64;
                if u32_at(&b, o + 4) == 0 { cies.push(at) } else { fdes.push(at) }
                o += 4 + len;
            }
            let mut bad = 0;
            let mut err = |m: String| { println!("  错误：{m}"); bad += 1; };
            if n != fdes.len() { err(format!("fde_count = {n}，.eh_frame 里实有 {} 条 FDE", fdes.len())) }
            for i in 0..n {
                let (loc, fde) = entry(i);
                if i > 0 && entry(i - 1).0 >= loc { err(format!("[{i}] {loc:#x} 不大于前一项，表没有严格递增")) }
                if cies.contains(&fde) { err(format!("[{i}] 指向 {fde:#x}，那是一条 CIE")); continue }
                if !fdes.contains(&fde) { err(format!("[{i}] 指向 {fde:#x}，不在记录边界上")); continue }
                let (_, pc_begin, _) = read_fde(fde);
                if pc_begin != loc { err(format!("[{i}] initial_location = {loc:#x}，FDE 的 pc_begin = {pc_begin:#x}")) }
            }
            if bad > 0 { eprintln!("ehhdr: --check 发现 {bad} 处错误"); exit(1) }
            println!("  --check 通过：{n} 项，有序，与 .eh_frame 一致");
        }
        Some(s) => {
            let pc = u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or_else(|_| die("PC 要写成十六进制"));
            // 二分：找最后一个 initial_location <= pc 的表项
            let (mut lo, mut hi) = (0usize, n);
            while lo < hi { let mid = (lo + hi) / 2; if entry(mid).0 <= pc { lo = mid + 1 } else { hi = mid } }
            if lo == 0 { println!("{pc:#x}: 比第一项还小，没有 FDE"); return }
            let (_, fde) = entry(lo - 1);
            let (cie, pc_begin, pc_range) = read_fde(fde);
            let note = if pc < pc_begin + pc_range { "" } else { "（不在范围内：没有覆盖它的 FDE）" };
            println!("{pc:#x}: 表项 [{}], FDE @ {fde:#x}, CIE @ {cie:#x}, pc = {pc_begin:#x}..{:#x}{note}",
                     lo - 1, pc_begin + pc_range);
        }
    }
}
