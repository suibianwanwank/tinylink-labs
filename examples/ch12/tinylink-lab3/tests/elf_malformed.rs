//! 畸形输入：截断、越界的偏移和下标、随机改坏的字节。
//!
//! 输入文件是不可信的。不管字节怎么乱，解析和链接都只能返回错误，不能 panic。

use tinylink::elf::error::StringProblem;
use tinylink::elf::{ElfError, ElfFile, Place};
use tinylink::input::InputName;
use tinylink::link::link;
use tinylink::nm::NmView;
use tinylink::readelf::{HeaderView, RelocsView, SectionsView, SegmentsView, SymbolsView};
use tinylink::resolve::{InputFile, ResolveView, resolve};

const MAIN_O: &[u8] = include_bytes!("elf/fixtures/main.o");

const FIXTURES: &[&[u8]] = &[
    MAIN_O,
    include_bytes!("elf/fixtures/add.o"),
    include_bytes!("elf/fixtures/bind.o"),
    include_bytes!("elf/fixtures/quiz.o"),
    include_bytes!("elf/fixtures/predict.o"),
    include_bytes!("elf/fixtures/predict-gas.o"),
    include_bytes!("elf/fixtures/exit42.o"),
    include_bytes!("elf/fixtures/exit42-a64.o"),
    include_bytes!("elf/fixtures/common.o"),
    include_bytes!("elf/fixtures/exit42-lld.elf"),
    include_bytes!("elf/fixtures/exit42-gnu.elf"),
];

/// main.o 的节头表从 0x2f0 开始，每项 64 字节（见 expected/main.sections）。
const SHOFF: usize = 0x2f0;
/// 节头里各字段的偏移。
const SH_TYPE: usize = 4;
const SH_SIZE: usize = 32;
const SH_LINK: usize = 40;
const SH_ENTSIZE: usize = 56;
/// main.o 里几个节的编号。
const STRTAB: usize = 1;
const RELA_TEXT: usize = 3;
const SYMTAB: usize = 14;

fn section_field(index: usize, field: usize) -> usize {
    SHOFF + index * 64 + field
}

fn patched(offset: usize, bytes: &[u8]) -> Vec<u8> {
    let mut data = MAIN_O.to_vec();
    data[offset..offset + bytes.len()].copy_from_slice(bytes);
    data
}

/// 把一个文件能走的路径全走一遍：解析、五种 readelf 视图、nm，和 add.o 一起做符号解析，再链接。
fn exercise(data: &[u8]) {
    let Ok(file) = ElfFile::parse(data) else {
        return;
    };
    let _ = HeaderView(&file).to_string();
    let _ = SectionsView(&file).to_string();
    let _ = SegmentsView(&file).to_string();
    if let Ok(v) = SymbolsView::new(&file) {
        let _ = v.to_string();
    }
    if let Ok(v) = RelocsView::new(&file) {
        let _ = v.to_string();
    }
    if let Ok(v) = NmView::new(&file) {
        let _ = v.to_string();
    }
    let name = |n: &str| InputName::Path(n.to_owned());
    let Ok(input) = InputFile::parse(name("mutated.o"), data) else {
        return;
    };
    let add = InputFile::parse(name("add.o"), include_bytes!("elf/fixtures/add.o"))
        .expect("add.o is valid");
    let inputs = [input, add];
    let resolution = resolve(&inputs);
    let _ = ResolveView::new(&inputs, &resolution).to_string();
    for d in &resolution.diags {
        let _ = d.display(&inputs, &resolution).to_string();
    }
    if !resolution.has_errors() {
        let _ = link(&inputs, &resolution);
    }
}

#[test]
fn every_truncation_is_an_error() {
    // main.o 的每一个真前缀都不完整：要么 ELF 头不全，要么节头表或某个节的内容被截掉。
    for len in 0..MAIN_O.len() {
        assert!(
            ElfFile::parse(&MAIN_O[..len]).is_err(),
            "prefix of {len} bytes parsed"
        );
    }
    assert!(ElfFile::parse(MAIN_O).is_ok());
}

#[test]
fn truncated_ehdr_reports_position() {
    let err = ElfFile::parse(&MAIN_O[..10]).unwrap_err();
    assert_eq!(
        err,
        ElfError::OutOfBounds {
            place: Place::FileHeader,
            offset: 0,
            size: 64,
            file_len: 10
        }
    );
}

#[test]
fn shoff_past_end() {
    let data = patched(0x28, &0xffff_ffff_ffff_0000u64.to_le_bytes());
    let err = ElfFile::parse(&data).unwrap_err();
    assert!(
        matches!(
            err,
            ElfError::OutOfBounds {
                place: Place::SectionHeaderTable,
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn section_data_past_end() {
    // 2 号节（.text）的 sh_offset 在节头的第 24 字节。
    let data = patched(0x2f0 + 2 * 64 + 24, &0x10_0000u64.to_le_bytes());
    let err = ElfFile::parse(&data).unwrap_err();
    assert_eq!(
        err.to_string(),
        "contents of section 2: bytes 0x100000..0x100015 are outside the file (0x6b0 bytes)"
    );
}

#[test]
fn sh_name_past_string_table() {
    let data = patched(0x2f0 + 2 * 64, &0x10000u32.to_le_bytes());
    let err = ElfFile::parse(&data).unwrap_err();
    assert_eq!(
        err.to_string(),
        "section header 2: sh_name 0x10000 is outside the string table (0xa1 bytes)"
    );
}

#[test]
fn shstrndx_out_of_range() {
    let data = patched(0x3e, &99u16.to_le_bytes());
    let err = ElfFile::parse(&data).unwrap_err();
    assert_eq!(
        err.to_string(),
        "ELF header: e_shstrndx 99 is out of range (there are 15)"
    );
}

#[test]
fn relocation_symbol_index_out_of_range() {
    let data = patched(0x1e8 + 12, &99u32.to_le_bytes());
    let file = ElfFile::parse(&data).expect("headers are still fine");
    let relas = file.rela_sections().expect("only SHT_RELA");
    let rela = relas.first().expect(".rela.text");
    let err = file.relocations(rela).unwrap_err();
    assert_eq!(
        err.to_string(),
        "relocation 0 in section 3: symbol index 99 is out of range (there are 10)"
    );
}

#[test]
fn symbol_section_index_out_of_range() {
    // .symtab 在 0xf8，4 号符号（main）的 st_shndx 在这一项的第 6 字节。
    let data = patched(0xf8 + 4 * 24 + 6, &200u16.to_le_bytes());
    let file = ElfFile::parse(&data).unwrap();
    let err = file.symbol_table().unwrap_err();
    assert_eq!(
        err.to_string(),
        "symbol 4: st_shndx 200 is out of range (there are 15)"
    );
}

#[test]
fn wrong_class_and_endianness() {
    let err = ElfFile::parse(&patched(4, &[1])).unwrap_err();
    assert_eq!(
        err.to_string(),
        "ELF header: unsupported EI_CLASS (only 64-bit is supported) 0x1"
    );
    let err = ElfFile::parse(&patched(5, &[2])).unwrap_err();
    assert_eq!(
        err.to_string(),
        "ELF header: unsupported EI_DATA (only little-endian is supported) 0x2"
    );
}

/// 一个很小的伪随机数生成器（xorshift64），种子固定，每次运行改坏的字节都一样。
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// 随机变异的轮数。要多跑可以调大，调试构建会检查整数溢出，在那里跑最有意义。
const MUTATION_ROUNDS: usize = 5000;
/// 至少要有这么多个变异文件仍能解析成功，否则后面的 readelf、nm、链接根本没被执行到。
const MIN_PARSED: usize = 1000;

#[test]
fn random_mutations_never_panic() {
    let mut rng = XorShift(0x5eed_1234_abcd_ef01);
    let mut parsed = 0;
    for round in 0..MUTATION_ROUNDS {
        let original = FIXTURES[round % FIXTURES.len()];
        let mut data = original.to_vec();
        // 随机改 1 到 8 个字节。一半的时候改成"有意思"的值：0、0xff、刚好越界的大小。
        for _ in 0..1 + rng.below(8) {
            let at = rng.below(data.len());
            data[at] = match rng.below(4) {
                0 => 0,
                1 => 0xff,
                2 => 0x80,
                _ => rng.next() as u8,
            };
        }
        // 四分之一的时候再截掉一段尾巴。
        if rng.below(4) == 0 {
            let len = rng.below(data.len());
            data.truncate(len);
        }
        if ElfFile::parse(&data).is_ok() {
            parsed += 1;
        }
        exercise(&data);
    }
    // 大部分变异落在节内容里，文件结构仍然完好。确认测试真的走到了后面的路径。
    assert!(parsed > MIN_PARSED, "only {parsed} mutated files parsed");
}

#[test]
fn shstrndx_zero_means_no_section_names() {
    // gABI 允许 e_shstrndx 为 SHN_UNDEF：文件没有节名字符串表。GNU readelf 把节名显示成 <no-strings>。
    let data = patched(0x3e, &0u16.to_le_bytes());
    let file = ElfFile::parse(&data).expect("still a valid file");
    assert!(!file.has_section_names());
    assert!(file.sections().iter().all(|s| s.name.is_empty()));
    let table = SectionsView(&file).to_string();
    assert!(
        table.contains("  [ 2] <no-strings>      PROGBITS "),
        "{table}"
    );
}

#[test]
fn shstrndx_must_point_to_a_string_table() {
    let data = patched(0x3e, &2u16.to_le_bytes());
    let err = ElfFile::parse(&data).unwrap_err();
    assert_eq!(
        err.to_string(),
        "ELF header: e_shstrndx points to section 2 of type PROGBITS, expected STRTAB"
    );
}

#[test]
fn section_name_table_changed_to_nobits() {
    // NOBITS 节在文件里没有内容，不能当字符串表用。
    let data = patched(section_field(STRTAB, SH_TYPE), &8u32.to_le_bytes());
    let err = ElfFile::parse(&data).unwrap_err();
    assert_eq!(
        err.to_string(),
        "ELF header: e_shstrndx points to section 1 of type NOBITS, expected STRTAB"
    );
}

#[test]
fn symtab_link_must_be_a_string_table() {
    let data = patched(section_field(SYMTAB, SH_LINK), &2u32.to_le_bytes());
    let file = ElfFile::parse(&data).expect("headers are fine");
    let err = file.symbol_table().unwrap_err();
    assert_eq!(
        err.to_string(),
        "section header 14: sh_link points to section 2 of type PROGBITS, expected STRTAB"
    );
}

#[test]
fn rela_link_must_be_a_symbol_table() {
    let data = patched(
        section_field(RELA_TEXT, SH_LINK),
        &(STRTAB as u32).to_le_bytes(),
    );
    let file = ElfFile::parse(&data).expect("headers are fine");
    let err = RelocsView::new(&file).err().expect("bad sh_link");
    assert_eq!(
        err.to_string(),
        "section header 3: sh_link points to section 1 of type STRTAB, expected SYMTAB"
    );
}

#[test]
fn symtab_entry_size_must_be_24() {
    for entsize in [0u64, 16] {
        let data = patched(section_field(SYMTAB, SH_ENTSIZE), &entsize.to_le_bytes());
        let file = ElfFile::parse(&data).expect("headers are fine");
        let err = file.symbol_table().unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("section header 14: sh_entsize is {entsize}, expected 24")
        );
    }
}

#[test]
fn symtab_size_must_be_a_multiple_of_entry_size() {
    let data = patched(section_field(SYMTAB, SH_SIZE), &0xefu64.to_le_bytes());
    let file = ElfFile::parse(&data).expect("headers are fine");
    let err = file.symbol_table().unwrap_err();
    assert_eq!(
        err.to_string(),
        "section header 14: sh_size 0xef is not a multiple of sh_entsize 24"
    );
}

#[test]
fn extended_numbering_names_the_real_trigger() {
    let data = patched(0x3c, &0u16.to_le_bytes());
    let err = ElfFile::parse(&data).unwrap_err();
    assert_eq!(
        err.to_string(),
        "ELF header: extended section numbering is not supported yet \
         (e_shnum is 0 but e_shoff is 0x2f0, so the count is in sh_size of section header 0)"
    );

    let data = patched(0x3e, &0xffffu16.to_le_bytes());
    let err = ElfFile::parse(&data).unwrap_err();
    assert_eq!(
        err.to_string(),
        "ELF header: extended section numbering is not supported yet \
         (e_shstrndx is 0xffff, SHN_XINDEX)"
    );
}

#[test]
fn string_table_without_final_nul() {
    // .strtab 在 0x248，大小 0xa1：把最后一个字节（最后一个名字的 NUL）改掉。
    let data = patched(0x248 + 0xa1 - 1, b"x");
    let err = ElfFile::parse(&data)
        .and_then(|file| file.symbol_table().map(drop))
        .unwrap_err();
    assert!(
        matches!(
            err,
            ElfError::BadString {
                problem: StringProblem::Unterminated,
                ..
            }
        ),
        "{err}"
    );
    assert!(
        err.to_string()
            .ends_with("points to a string with no terminating NUL"),
        "{err}"
    );
}

#[test]
fn nm_shows_size_of_common_symbols() {
    // COMMON 符号的 st_value 是对齐，GNU nm 在第一列显示的是 st_size。
    let data = include_bytes!("elf/fixtures/common.o");
    let file = ElfFile::parse(data).unwrap();
    let out = NmView::new(&file).unwrap().to_string();
    assert_eq!(
        out,
        "0000000000000004 C c\n0000000000000028 C d\n0000000000000000 D defined\n"
    );
}

#[test]
fn rel_sections_are_reported_as_unsupported() {
    // 把 .rela.text 的类型从 SHT_RELA（4）改成 SHT_REL（9）。
    let data = patched(section_field(RELA_TEXT, SH_TYPE), &9u32.to_le_bytes());
    let file = ElfFile::parse(&data).expect("headers are fine");
    let err = file.rela_sections().unwrap_err();
    assert_eq!(
        err.to_string(),
        "section header 3: unsupported sh_type SHT_REL (only SHT_RELA is supported) 0x9"
    );
}

#[test]
fn rela_linked_to_dynsym_is_unsupported() {
    // 把 .symtab 的类型改成 SHT_DYNSYM（11），.rela.text 的 sh_link 仍然指向它。
    let data = patched(section_field(SYMTAB, SH_TYPE), &11u32.to_le_bytes());
    let file = ElfFile::parse(&data).expect("headers are fine");
    let err = RelocsView::new(&file).err().expect("dynsym link");
    assert_eq!(
        err.to_string(),
        "section header 3: unsupported sh_link to SHT_DYNSYM 0xe"
    );
}
