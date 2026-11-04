# RISC-V: strlen(a0) -> a0
    .text
    .globl  strlen
    .align  2
strlen:
    mv      t0, a0              # t0 = cursor
1:
    lbu     t1, 0(t0)
    beqz    t1, 2f
    addi    t0, t0, 1
    j       1b
2:
    sub     a0, t0, a0
    ret

    .globl  main
main:
    addi    sp, sp, -16
    sd      ra, 8(sp)
    lui     a0, %hi(greeting)
    addi    a0, a0, %lo(greeting)
    call    strlen
    li      a7, 93              # exit syscall
    fadd.d  fa0, fa1, ft2
    csrr    t0, mstatus
    ld      ra, 8(sp)
    addi    sp, sp, 16
    ecall

    .section .rodata
greeting:
    .string "hello, risc-v\n"
    .word   0x10, 0b1010
