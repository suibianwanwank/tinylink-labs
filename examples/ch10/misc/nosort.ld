ENTRY(_entry)
SECTIONS
{
  . = 0x80000000;
  .text : { *(.text .text.*) }
  .data : {
    . = ALIGN(8);
    __initcall_start = .;
    KEEP(*(.initcall*))
    __initcall_end = .;
    *(.data .data.* .sdata .sdata.*)
  }
  .empty : { marker = .; }
  /DISCARD/ : { *(.comment) *(.riscv.attributes) }
  ASSERT(__initcall_end - __initcall_start == 3 * 8, "expected 3 initcalls")
}
