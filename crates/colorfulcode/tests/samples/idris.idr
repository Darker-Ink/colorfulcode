module Main

import Data.Vect
import Data.String

%default total

||| A length indexed vector append.
append : Vect n a -> Vect m a -> Vect (n + m) a
append [] ys = ys
append (x :: xs) ys = x :: append xs ys

{- Records and interfaces -}
record Point where
  constructor MkPoint
  x : Double
  y : Double

interface Shape s where
  area : s -> Double

data Circle = MkCircle Double

implementation Shape Circle where
  area (MkCircle r) = 3.14159 * r * r

-- Dependent pairs and case.
describe : Nat -> String
describe n = case n of
  Z => "zero"
  S k => "successor of " ++ show k

greet : String -> String
greet name = "Hello, " ++ name ++ "!\n"

partial
loop : IO ()
loop = do
  line <- getLine
  if line == "quit" then pure () else do
    putStrLn (greet line)
    loop

main : IO ()
main = do
  printLn (append [1, 2] [3, 0x10])
  putStrLn (describe 3 ++ show 'c')
