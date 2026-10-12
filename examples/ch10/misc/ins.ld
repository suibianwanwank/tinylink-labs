SECTIONS
{
  .mytable : { __tbl_start = .; KEEP(*(.mytable)) __tbl_end = .; }
}
INSERT AFTER .text;
