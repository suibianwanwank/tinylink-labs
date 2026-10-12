ENTRY(_entry)
MEMORY
{
  ROM (rx)  : ORIGIN = 0x80000000, LENGTH = 64K
  RAM (rwx) : ORIGIN = 0x80100000, LENGTH = 64K
}
SECTIONS
{
  .text   : { *(.text.entry) *(.text .text.*) } > ROM
  .rodata : { *(.rodata .rodata.* .srodata .srodata.*) } > ROM
  .data   : ALIGN(8) {
    _sdata = .;
    *(.data .data.* .sdata .sdata.*)
    . = ALIGN(8);
    _edata = .;
  } > RAM AT> ROM
  _sidata = LOADADDR(.data);
  .bss (NOLOAD) : ALIGN(8) {
    _sbss = .;
    *(.bss .bss.* .sbss .sbss.*)
    . = ALIGN(8);
    _ebss = .;
  } > RAM
  stack_top = ORIGIN(RAM) + LENGTH(RAM);
}
