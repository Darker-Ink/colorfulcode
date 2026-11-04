module Main where

import Prelude

import Data.Array (filter, length)
import Data.Maybe (Maybe(..), fromMaybe)
import Effect (Effect)
import Effect.Console (log)

-- | A user record.
type User = { name :: String, age :: Int }

data Shape
  = Circle Number
  | Rect Number Number

class Describe a where
  describe :: a -> String

instance describeShape :: Describe Shape where
  describe (Circle r) = "circle of radius " <> show r
  describe (Rect w h) = "rect " <> show w <> "x" <> show h

area :: Shape -> Number
area = case _ of
  Circle r -> 3.14159 * r * r
  Rect w h -> w * h

adults :: Array User -> Int
adults users = length (filter (\u -> u.age >= 18) users)

safeDiv :: Int -> Int -> Maybe Int
safeDiv _ 0 = Nothing
safeDiv a b = Just (a / b)

{- Raw strings keep backslashes: -}
pattern' :: String
pattern' = """\d+\s*"""

main :: Effect Unit
main = do
  log $ describe (Circle 2.0)
  log $ "adults: " <> show (adults [ { name: "Ann", age: 31 } ])
  log $ show (fromMaybe 0 (safeDiv 10 2)) <> "\n"
  when (area (Rect 2.0 3.0) > 5.0) do
    log "big"
