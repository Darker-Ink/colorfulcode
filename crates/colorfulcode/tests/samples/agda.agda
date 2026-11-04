{-# OPTIONS --safe #-}
module Nat where

open import Relation.Binary.PropositionalEquality using (_≡_; refl; cong)

-- Natural numbers, defined inductively.
data ℕ : Set where
  zero : ℕ
  suc  : ℕ → ℕ

{-# BUILTIN NATURAL ℕ #-}

_+_ : ℕ → ℕ → ℕ
zero  + n = n
suc m + n = suc (m + n)

infixl 6 _+_

{- Proofs are programs.
   {- Comments nest. -} -}
+-identityʳ : ∀ (n : ℕ) → n + zero ≡ n
+-identityʳ zero    = refl
+-identityʳ (suc n) = cong suc (+-identityʳ n)

record Pair (A B : Set) : Set where
  constructor _,_
  field
    fst : A
    snd : B

open Pair

swap : ∀ {A B : Set} → Pair A B → Pair B A
swap p = snd p , fst p

postulate
  String : Set
  greeting : String

{-# BUILTIN STRING String #-}

hello : String
hello = "hello, \"agda\"\n"

two : ℕ
two = 2
