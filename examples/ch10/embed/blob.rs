#![no_std]
#[no_mangle]
pub static BLOB: &[u8] = include_bytes!("data/msg-v1.txt");
