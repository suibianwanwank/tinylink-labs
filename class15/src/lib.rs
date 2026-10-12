//! A self-contained linker snapshot.

pub mod archive;
pub mod debug;
pub mod driver;
pub mod elf_header;
pub mod executable;
pub mod fuzz;
pub mod gc;
pub mod icf;
pub mod layout;
pub mod pie;
pub mod relocate;
pub mod resolve;
pub mod sections;
pub mod symbols;
pub mod tls;
pub mod unwind;

pub use fuzz::*;
