\ Classic Forth examples
( Stack comments describe inputs -- outputs )

variable counter
42 constant answer
create buffer 64 allot
0 value total

: square ( n -- n*n )  dup * ;
: cube   ( n -- n^3 )  dup square * ;

: countdown ( n -- )
  begin
    dup . 1-
    dup 0=
  until drop ;

: fizzbuzz ( n -- )
  1+ 1 ?do
    i 15 mod 0= if ." FizzBuzz" else
    i 3 mod 0= if ." Fizz" else
    i 5 mod 0= if ." Buzz" else
    i . then then then cr
  loop ;

: greet ( -- )  s" Hello, world" type cr ;
: hex-demo ( -- )  $FF . %1010 . #99 . -17 . 3.14e0 f. ;

: classify ( n -- )
  case
    0 of ." zero" endof
    1 of ." one" endof
    ." many"
  endcase ;

10 counter !
counter @ square .
answer cube to total
.( compile time message) cr
' square execute
