//! icfhash：读入若干 ELF64 小端可重定位文件，找出可以做 ICF 的代码节。
//!
//! 用法：icfhash [--safe] a.o [b.o ...]
//!
//! ELF 解析和符号解析复用 tinylink 实验 3 的代码（`tinylink::resolve`），
//! 所以重定位目标的定义按第 3 章的规则选：强定义压过弱定义，两个弱定义取先读到的。
//!
//! 步骤：
//! 1. COMDAT 去重：同一个组签名只保留第一个组，其余组的节不参与；
//! 2. 选候选节：可分配、可执行、不可写、非空的 PROGBITS 节。
//!    --safe 时排除 .llvm_addrsig 里列出的符号所在的节，没有 .llvm_addrsig 的文件整个排除；
//!    和 lld 一样，被带 LSDA 的 .eh_frame FDE 引用的节也排除；
//! 3. 先按"节标志 + 内容 + 重定位条数"算哈希分类，再把每条重定位的位置、类型、加数和
//!    "目标所在的等价类"并进哈希，反复细分，直到等价类个数不再增加；
//! 4. 对每个类做一次精确比较（内容逐字节、重定位逐条），把哈希碰撞拆开；拆过就回到第 3 步。
//!
//! 输出格式与 `ld.lld --print-icf-sections` 相同。每个类保留输入顺序里最靠前的节。

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use tinylink::elf::types::{SectionFlags, SectionIndex, SectionType};
use tinylink::input::InputName;
use tinylink::resolve::{Definition, InputFile, Resolution, resolve};

const GRP_COMDAT: u32 = 1;

/// 一个节：第几个输入文件的第几个节。
type SecId = (usize, usize);

/// 一条重定位的目标。
#[derive(Clone, PartialEq, Eq, Hash)]
enum Target {
    /// 落在某个输入节里，节内偏移（符号值加加数）。
    Sec(SecId, i64),
    /// 没有定义的外部符号（或者 ABS、COMMON 这类不在节里的定义）。
    Other(Vec<u8>, i64),
}

/// 一条重定位：在节内的偏移、类型、目标。
#[derive(Clone, PartialEq, Eq, Hash)]
struct Reloc {
    offset: u64,
    kind: u32,
    target: Target,
}

fn hash_of<T: Hash>(t: &T) -> u64 {
    let mut h = DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

/// 重定位里的符号指向哪里。全局符号查 `Resolution`，局部符号直接用它所在的节。
fn target_of(
    inputs: &[InputFile],
    res: &Resolution,
    file: usize,
    sym: usize,
    addend: i64,
) -> Target {
    let symbol = &inputs[file].symbols[sym];
    if let Some(Some(id)) = res.ids[file].get(sym) {
        let global = res.table.get(*id);
        if let Definition::Defined {
            at,
            section: SectionIndex::Index(n),
            value,
            ..
        } = global.definition
        {
            return Target::Sec((at.file, n), value as i64 + addend);
        }
        return Target::Other(global.name.as_bytes().to_vec(), addend);
    }
    match symbol.section {
        SectionIndex::Index(n) => Target::Sec((file, n), symbol.value as i64 + addend),
        _ => Target::Other(symbol.name.as_bytes().to_vec(), addend),
    }
}

/// COMDAT 去重：返回被丢弃的节。组的内容是一个 u32 标志，后面是成员节号。
fn discarded_by_comdat(inputs: &[InputFile]) -> HashSet<SecId> {
    let mut seen: HashSet<Vec<u8>> = HashSet::new();
    let mut out = HashSet::new();
    for (f, input) in inputs.iter().enumerate() {
        for s in input.elf.sections() {
            if s.header.kind != SectionType::Group || s.data.len() < 4 {
                continue;
            }
            let words: Vec<u32> = s
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes(*c))
                .collect();
            if words[0] & GRP_COMDAT == 0 {
                continue;
            }
            // 组签名是 sh_info 指向的那个符号的名字。
            let Some(sig) = input.symbols.get(s.header.info as usize) else {
                continue;
            };
            if !seen.insert(sig.name.as_bytes().to_vec()) {
                out.insert((f, s.index));
                for &m in &words[1..] {
                    out.insert((f, m as usize));
                }
            }
        }
    }
    out
}

/// 被带 LSDA 的 FDE 引用的节。最小的 .eh_frame 解析：CIE 的增强串里有 'L'，
/// 它名下的每个 FDE 就带 LSDA 指针（第 8 章）；FDE 偏移 8 处的 pc_begin
/// 由一条重定位指向函数所在的节。完整的解析在实验 9。
fn sections_with_lsda(input: &InputFile, file: usize) -> Vec<SecId> {
    let mut out = Vec::new();
    let Some(eh) = input.elf.sections().iter().find(|s| s.name == ".eh_frame") else {
        return out;
    };
    let d = eh.data;
    let mut cie_has_l: HashMap<usize, bool> = HashMap::new();
    let mut pos = 0;
    while pos + 8 <= d.len() {
        let len = u32::from_le_bytes([d[pos], d[pos + 1], d[pos + 2], d[pos + 3]]) as usize;
        if len == 0 || len == 0xffff_ffff {
            break; // 终止项；64 位长度的记录这里不处理
        }
        let id = u32::from_le_bytes([d[pos + 4], d[pos + 5], d[pos + 6], d[pos + 7]]) as usize;
        if id == 0 {
            // CIE：4 字节长度、4 字节 ID、1 字节版本，然后是以 0 结尾的增强串。
            let aug = &d[pos + 9..];
            let end = aug.iter().position(|&b| b == 0).unwrap_or(0);
            cie_has_l.insert(pos, aug[..end].contains(&b'L'));
        } else if let Some(cie) = (pos + 4).checked_sub(id)
            && cie_has_l.get(&cie).copied().unwrap_or(false)
        {
            let pc_begin = (pos + 8) as u64;
            if let Some(r) = input
                .relocations
                .iter()
                .find(|r| r.section == eh.index && r.rela.offset == pc_begin)
                && let SectionIndex::Index(n) = input.symbols[r.rela.symbol as usize].section
            {
                out.push((file, n));
            }
        }
        pos += 4 + len;
    }
    out
}

fn main() {
    let mut safe = false;
    let mut paths = Vec::new();
    for a in std::env::args().skip(1) {
        if a == "--safe" {
            safe = true;
        } else {
            paths.push(a);
        }
    }
    if paths.is_empty() {
        eprintln!("用法：icfhash [--safe] a.o [b.o ...]");
        std::process::exit(2);
    }
    let data: Vec<Vec<u8>> = paths
        .iter()
        .map(|p| {
            std::fs::read(p).unwrap_or_else(|e| {
                eprintln!("{p}: {e}");
                std::process::exit(1)
            })
        })
        .collect();
    let inputs: Vec<InputFile> = paths
        .iter()
        .zip(&data)
        .map(|(p, d)| {
            InputFile::parse(InputName::Path(p.clone()), d).unwrap_or_else(|e| {
                eprintln!("{p}: {e}");
                std::process::exit(1)
            })
        })
        .collect();
    let res = resolve(&inputs);

    // 1. COMDAT 去重；2. 不参与 ICF 的节
    let mut keep_unique = discarded_by_comdat(&inputs);
    for (f, input) in inputs.iter().enumerate() {
        keep_unique.extend(sections_with_lsda(input, f));
        if !safe {
            continue;
        }
        let addrsig = input
            .elf
            .sections()
            .iter()
            .find(|s| s.header.kind == SectionType::LlvmAddrsig);
        match addrsig {
            Some(s) => {
                let (mut pos, d) = (0, s.data);
                while pos < d.len() {
                    // ULEB128：地址被取用的符号的下标
                    let (mut v, mut shift) = (0usize, 0);
                    loop {
                        let b = d[pos];
                        pos += 1;
                        v |= ((b & 0x7f) as usize) << shift;
                        if b & 0x80 == 0 {
                            break;
                        }
                        shift += 7;
                    }
                    if let Target::Sec(sec, _) = target_of(&inputs, &res, f, v, 0) {
                        keep_unique.insert(sec);
                    }
                }
            }
            None => keep_unique.extend((0..input.elf.sections().len()).map(|n| (f, n))),
        }
    }
    let mut cands: Vec<SecId> = Vec::new();
    for (f, input) in inputs.iter().enumerate() {
        for s in input.elf.sections() {
            let fl = s.header.flags;
            if s.header.kind == SectionType::Progbits
                && fl.contains(SectionFlags::ALLOC)
                && fl.contains(SectionFlags::EXECINSTR)
                && !fl.contains(SectionFlags::WRITE)
                && s.header.size > 0
                && !keep_unique.contains(&(f, s.index))
            {
                cands.push((f, s.index));
            }
        }
    }
    let index: HashMap<SecId, usize> = cands.iter().enumerate().map(|(i, &k)| (k, i)).collect();
    let relocs: Vec<Vec<Reloc>> = cands
        .iter()
        .map(|&(f, n)| {
            let mut v: Vec<Reloc> = inputs[f]
                .relocations
                .iter()
                .filter(|r| r.section == n)
                .map(|r| Reloc {
                    offset: r.rela.offset,
                    kind: r.rela.kind,
                    target: target_of(&inputs, &res, f, r.rela.symbol as usize, r.rela.addend),
                })
                .collect();
            v.sort_by_key(|r| r.offset);
            v
        })
        .collect();
    let section = |(f, n): SecId| inputs[f].elf.section(n).expect("candidate exists");

    // 等价类用从 0 开始的编号表示，哈希只用来分组，不直接当编号：
    // 新编号由（旧编号，哈希）决定，所以每一轮只会把类拆细，不会把两个类并起来。
    fn renumber(keys: impl Iterator<Item = (usize, u64)>) -> Vec<usize> {
        let mut ids: HashMap<(usize, u64), usize> = HashMap::new();
        keys.map(|k| {
            let next = ids.len();
            *ids.entry(k).or_insert(next)
        })
        .collect()
    }
    let count = |c: &[usize]| c.iter().collect::<HashSet<_>>().len();
    // 重定位目标在 ICF 意义上的身份：候选节用它当前的类，其他节用它自己
    let key_of = |t: &Target, class: &[usize]| -> (u8, u64, i64) {
        match t {
            Target::Sec(sec, a) => match index.get(sec) {
                Some(&j) => (0, class[j] as u64, *a),
                None => (1, hash_of(sec), *a),
            },
            Target::Other(name, a) => (2, hash_of(name), *a),
        }
    };

    // 3. 第一轮：只看标志、内容和重定位条数
    let mut class = renumber(cands.iter().zip(&relocs).map(|(&id, r)| {
        let s = section(id);
        (0, hash_of(&(s.header.flags.0, s.data, r.len())))
    }));
    let mut rounds = 1;
    let mut classes = count(&class);
    loop {
        // 细分到不再变化
        loop {
            let next = renumber((0..cands.len()).map(|i| {
                let keys: Vec<_> = relocs[i]
                    .iter()
                    .map(|r| (r.offset, r.kind, key_of(&r.target, &class)))
                    .collect();
                (class[i], hash_of(&keys))
            }));
            rounds += 1;
            let c = count(&next);
            class = next;
            if c == classes {
                break;
            }
            classes = c;
        }
        // 4. 精确比较：同一个类里，和每个已有代表逐字节、逐条比较，不相等就分到新编号
        let same = |i: usize, j: usize, class: &[usize]| {
            let (a, b) = (section(cands[i]), section(cands[j]));
            a.header.flags == b.header.flags
                && a.data == b.data
                && relocs[i].len() == relocs[j].len()
                && relocs[i].iter().zip(&relocs[j]).all(|(x, y)| {
                    x.offset == y.offset
                        && x.kind == y.kind
                        && key_of(&x.target, class) == key_of(&y.target, class)
                })
        };
        let mut reps: HashMap<usize, Vec<(usize, usize)>> = HashMap::new(); // 类 -> [(代表, 编号)]
        let mut split = class.clone();
        let mut next_id = classes;
        for i in 0..cands.len() {
            let list = reps.entry(class[i]).or_default();
            match list.iter().find(|&&(j, _)| same(i, j, &class)) {
                Some(&(_, id)) => split[i] = id,
                None => {
                    let id = if list.is_empty() { class[i] } else { next_id };
                    if !list.is_empty() {
                        next_id += 1;
                    }
                    list.push((i, id));
                    split[i] = id;
                }
            }
        }
        let c = count(&split);
        class = split;
        if c == classes {
            break;
        }
        classes = c;
    }

    // 输出：类按其中最靠前的节排序，每类第一个保留
    let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut pos_of: HashMap<usize, usize> = HashMap::new();
    for (i, &c) in class.iter().enumerate() {
        let p = *pos_of.entry(c).or_insert_with(|| {
            groups.push((c, Vec::new()));
            groups.len() - 1
        });
        groups[p].1.push(i);
    }
    let name = |i: usize| {
        let (f, n) = cands[i];
        format!("{}:({})", inputs[f].name, inputs[f].section_name(n))
    };
    let mut folded = 0;
    for (_, members) in groups.iter().filter(|(_, m)| m.len() > 1) {
        println!("selected section {}", name(members[0]));
        for &i in &members[1..] {
            println!("  removing identical section {}", name(i));
            folded += 1;
        }
    }
    eprintln!(
        "{} 个候选节，{} 轮后得到 {} 个等价类，可折叠 {} 个节{}",
        cands.len(),
        rounds,
        classes,
        folded,
        if safe { "（safe）" } else { "" }
    );
}
