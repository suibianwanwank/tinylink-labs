#![no_std]
#![no_main]
use core::fmt::Write;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    exit(101)
}

fn exit(code: i32) -> ! {
    unsafe { core::arch::asm!("syscall", in("rax") 231, in("rdi") code, options(noreturn)) }
}

struct Stdout;
impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let n: isize;
        unsafe {
            core::arch::asm!("syscall", inlateout("rax") 1isize => n, in("rdi") 1, in("rsi") s.as_ptr(),
                in("rdx") s.len(), lateout("rcx") _, lateout("r11") _, options(nostack));
        }
        if n as usize == s.len() { Ok(()) } else { Err(core::fmt::Error) }
    }
}

static PRIMES: [u32; 6] = [2, 3, 5, 7, 11, 13];

#[inline(never)]
fn pick(index: usize) -> u32 {
    PRIMES[core::hint::black_box(index)]
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let total: u32 = (0..PRIMES.len()).map(pick).sum::<u32>() + 1;
    let _ = writeln!(Stdout, "sum={total} {:?} {:.2}", &PRIMES[..2], 1.5f64);
    exit(total as i32)
}

// The program has no libc, so it supplies the memory routines that LLVM and
// core call. They are written in assembly so that the compiler cannot turn
// their loops back into calls to themselves.
core::arch::global_asm!(
    ".globl memcpy, memmove, memset, memcmp, bcmp",
    "memcpy:",
    "  mov rax, rdi",
    "  mov rcx, rdx",
    "  rep movsb",
    "  ret",
    "memmove:",
    "  mov rax, rdi",
    "  mov rcx, rdx",
    "  cmp rdi, rsi",
    "  jbe 2f",
    "  lea rsi, [rsi + rdx - 1]",
    "  lea rdi, [rdi + rdx - 1]",
    "  std",
    "  rep movsb",
    "  cld",
    "  ret",
    "2: rep movsb",
    "  ret",
    "memset:",
    "  mov r8, rdi",
    "  mov eax, esi",
    "  mov rcx, rdx",
    "  rep stosb",
    "  mov rax, r8",
    "  ret",
    "memcmp:",
    "bcmp:",
    "  xor eax, eax",
    "3: test rdx, rdx",
    "  jz 4f",
    "  movzx eax, byte ptr [rdi]",
    "  movzx ecx, byte ptr [rsi]",
    "  sub eax, ecx",
    "  jnz 4f",
    "  inc rdi",
    "  inc rsi",
    "  dec rdx",
    "  jmp 3b",
    "4: ret",
);
