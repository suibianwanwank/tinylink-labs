//! A self-contained linker snapshot.

pub mod elf_header;
pub mod executable;
pub mod resolve;
pub mod sections;
pub mod symbols;

pub use executable::*;
