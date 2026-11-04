# Factorial in GNU assembler, AT&T syntax
#include <asm/unistd.h>
    .file   "fact.c"
    .text
    .globl  factorial
    .type   factorial, @function
factorial:
    pushq   %rbp
    movq    %rsp, %rbp
    movl    %edi, -4(%rbp)      # save n
    cmpl    $1, -4(%rbp)
    jg      .L2
    movl    $1, %eax
    jmp     .L3
.L2:
    movl    -4(%rbp), %eax
    subl    $1, %eax
    movl    %eax, %edi
    call    factorial
    imull   -4(%rbp), %eax
.L3:
    popq    %rbp; ret
    .size   factorial, .-factorial

    .section .rodata
.LC0:
    .string "result: %d\n"
    .long   0x2A, -1
/* block comment
   spanning lines */
counter = 10
    movq    $counter, %rcx
    leaq    .LC0(%rip), %rdi
