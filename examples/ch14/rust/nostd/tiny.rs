// tiny.rs：不用标准库、不用 C 运行时的最小 Rust 程序
#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

fn write(fd: usize, buf: &[u8]) -> isize {
    let ret: isize;
    unsafe {
        asm!("syscall", inlateout("rax") 1isize => ret, in("rdi") fd,
             in("rsi") buf.as_ptr(), in("rdx") buf.len(),
             lateout("rcx") _, lateout("r11") _, options(nostack));
    }
    ret
}

fn exit(code: i32) -> ! {
    unsafe { asm!("syscall", in("rax") 60, in("rdi") code, options(noreturn, nostack)) }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { exit(101) }

// Linux enters this symbol by a jump, not a C ABI call. Establish a regular
// function-call frame before entering compiler-generated Rust code.
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        "and rsp, -16",
        "call {entry}",
        "ud2",
        entry = sym rust_entry,
    );
}

#[unsafe(no_mangle)]
extern "C" fn rust_entry() -> ! {
    write(1, b"hello from no_std\n");
    exit(42)
}
