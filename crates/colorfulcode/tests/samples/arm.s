// AArch64: sum an array of 32-bit integers
    .global sum_array
    .type   sum_array, %function
sum_array:                      // x0 = ptr, w1 = count
    mov     w2, #0
    cbz     w1, 2f
1:
    ldr     w3, [x0], #4
    add     w2, w2, w3
    subs    w1, w1, #1
    b.ne    1b
2:
    mov     w0, w2
    ret

@ ARMv7 (A32) with conditional execution
    .arm
gcd:
    cmp     r0, r1
    subgt   r0, r0, r1
    sublt   r1, r1, r0
    bne     gcd
    bx      lr

    ; armasm style comment
    stmfd   sp!, {r4-r7, lr}
    ldr     r4, =message
    add     r5, r4, r6, lsl #2
    vadd.f32 q0, q1, q2
    csel    x0, x1, x2, eq
    ldmfd   sp!, {r4-r7, pc}
message:
    .asciz  "Hello, ARM\n"
    .word   0xDEADBEEF, -16
