/* SPDX-License-Identifier: Apache-2.0 */

ENTRY(_reset)

MEMORY {
  ddr (rwx) : ORIGIN = 0x80000000, LENGTH = 0x00200000
}

SECTIONS {
  .text : {
    *(.text.startup)
    *(.text*)
  } > ddr

  .rodata : { *(.rodata*) } > ddr
  .data : { *(.data*) } > ddr

  .bss : {
    *(.bss*)
    *(COMMON)
  } > ddr

  . = ALIGN(16);
  . += 0x8000; /* 32KB stack */
  _stack_top = .;
}
