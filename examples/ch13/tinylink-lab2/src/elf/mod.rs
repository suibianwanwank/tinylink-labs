//! M1：手写的 ELF64 小端解析器，只用标准库。
//!
//! - `read`：从不可信的字节里安全地取切片、读整数；
//! - `error`：带位置的错误；
//! - `types`：各种编号的 enum 和它们在 readelf 里的名字；
//! - `name`：节名、符号名（原样的字节，显示时才转换）；
//! - `file`：把文件解析成 ELF 头、程序头、节、符号和重定位，以及把 ELF 头、程序头写回字节。

pub mod error;
mod file;
pub mod name;
pub mod read;
pub mod types;

pub use error::{ElfError, Place, Result};
pub use file::{
    EHDR_SIZE, ElfFile, Header, PHDR_SIZE, ProgramHeader, RELA_SIZE, Rela, SHDR_SIZE, SYM_SIZE,
    Section, SectionHeader, Symbol, SymbolTable,
};
pub use name::Name;
