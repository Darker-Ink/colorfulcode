; Hello world for Linux x86-64 (NASM syntax)
%define SYS_WRITE 1
%define SYS_EXIT  60

%macro print 2
    mov     rax, SYS_WRITE
    mov     rdi, 1
    mov     rsi, %1
    mov     rdx, %2
    syscall
%endmacro

section .data
    msg     db  "Hello, world!", 10, 0
    len     equ $ - msg
    table   dw  0x1F, 0FFh, 1010b, -42
    quote   db  'it''s', `tab\there`, 0

section .bss
    buffer  resb 64

section .text
    global  _start

_start:
    print   msg, len
    xor     ecx, ecx
.loop:
    mov     al, byte [buffer + rcx*2 + 4]
    cmp     al, 0
    je      .done              ; end of string
    inc     ecx
    rep movsb
    jmp     short .loop
.done:
    movaps  xmm0, oword [rel table]
    mov     eax, dword [rsp+8]
    mov     r8d, SYS_EXIT
    xor     edi, edi
    syscall
