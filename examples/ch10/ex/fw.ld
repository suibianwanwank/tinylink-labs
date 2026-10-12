MEMORY
{
  FLASH (rx)  : ORIGIN = 0x08000000, LENGTH = 256K
  RAM   (rwx) : ORIGIN = 0x20000000, LENGTH = 64K
}
SECTIONS
{
  .vectors : { KEEP(*(.vectors)) } > FLASH
  .text : {
    *(.text .text.*)
    . = ALIGN(4);
    _etext = .;
  } > FLASH
  .rodata : { *(.rodata .rodata.*) } > FLASH
  .data : {
    _sdata = .;
    *(.data .data.*)
    . = ALIGN(4);
    _edata = .;
  } > RAM AT> FLASH
  _sidata = LOADADDR(.data);
  .bss (NOLOAD) : {
    _sbss = .;
    *(.bss .bss.*)
    . = ALIGN(4);
    _ebss = .;
  } > RAM
  _estack = ORIGIN(RAM) + LENGTH(RAM);
}
