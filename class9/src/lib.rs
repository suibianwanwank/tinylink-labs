//! A self-contained linker snapshot.

pub mod archive;
pub mod elf_header;
pub mod executable;
pub mod gc;
pub mod layout;
pub mod pie;
pub mod relocate;
pub mod resolve;
pub mod sections;
pub mod symbols;

pub use pie::*;
