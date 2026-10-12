ENTRY(_entry)
SECTIONS
{
  . = 0x80000000;
  .text : { *(.text .text.*) }
  /DISCARD/ : { *(.init.text) }
}
