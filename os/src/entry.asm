# os/src/entry.asm
    .section .text.entry
    .global _start
_start:
    la sp, boot_stack_top
    call rust_main

    # stack
    .section .bss.stack
    .global boot_stack
boot_stack:
    # 64KB
    .space 4096 * 16
    .global boot_stack_top
boot_stack_top: