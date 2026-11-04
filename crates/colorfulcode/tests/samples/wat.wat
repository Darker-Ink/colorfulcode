;; A small WebAssembly module
(module
  (import "env" "log" (func $log (param i32)))
  (memory (export "memory") 1)
  (data (i32.const 16) "Hello\0a\00")
  (global $counter (mut i32) (i32.const 0))

  (type $binop (func (param i32 i32) (result i32)))

  (func $add (export "add") (param $a i32) (param $b i32) (result i32)
    local.get $a
    local.get $b
    i32.add)

  (; factorial, iterative
     (; nested comment ;) ;)
  (func $fac (param $n i64) (result i64)
    (local $acc i64)
    (local.set $acc (i64.const 1))
    (block $done
      (loop $again
        (br_if $done (i64.eqz (local.get $n)))
        (local.set $acc (i64.mul (local.get $acc) (local.get $n)))
        (local.set $n (i64.sub (local.get $n) (i64.const 1)))
        (br $again)))
    (call $log (i32.wrap_i64 (local.get $acc)))
    (i32.store offset=4 align=2 (i32.const 0) (i32.const 0xFF))
    (f64.const nan)
    drop
    local.get $acc)

  (start $main)
  (func $main
    (global.set $counter (call $add (i32.const 1) (i32.const -2)))))
