/* SPDX-License-Identifier: Apache-2.0 */
/* Minimal ARM32 entry stub: load-binary sets PC to _reset via the ELF entry
 * point but does not set up a stack pointer. Set SP to the top of the
 * reserved stack region defined in link.ld, then branch to the C entry
 * point (_start, in harness_main.c). */

.section .text.startup
.global _reset
.type _reset, %function
_reset:
    ldr sp, =_stack_top
    bl _start
1:  b 1b
.size _reset, . - _reset
