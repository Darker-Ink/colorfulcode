import Mathlib.Data.Nat.Basic

/-! # Lists and proofs
  /- Block comments nest. -/ -/

namespace Demo

/-- Our own list type. -/
inductive MyList (α : Type) where
  | nil : MyList α
  | cons : α → MyList α → MyList α
  deriving Repr

structure Point where
  x : Float
  y : Float
  deriving Repr

def MyList.length {α : Type} : MyList α → Nat
  | .nil => 0
  | .cons _ xs => 1 + xs.length

def greet (name : String) : String :=
  s!"Hello, {name}!\n"

@[simp] theorem add_zero' (n : Nat) : n + 0 = n := by
  induction n with
  | zero => rfl
  | succ k ih => simp

theorem two_eq : 1 + 1 = 2 := by decide

def sumTo : Nat → Nat
  | 0 => 0
  | n + 1 => (n + 1) + sumTo n

def firstChar (s : String) : Char :=
  s.get! 0 |>.toUpper

instance : Add Point where
  add p q := ⟨p.x + q.x, p.y + q.y⟩

end Demo

#eval Demo.greet "Lean"
#check Demo.two_eq
#eval (0xFF, 3.25, 'a', '\n', Demo.sumTo 10)
