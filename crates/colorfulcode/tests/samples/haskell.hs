{-# LANGUAGE OverloadedStrings #-}
{-# LANGUAGE ScopedTypeVariables #-}
-- | A tiny word frequency counter.
module Main (main) where

import qualified Data.Map.Strict as Map
import Data.Char (isAlpha, toLower)
import Data.List (sortBy)

{- Block comments {- nest -} properly. -}

-- | Records with field signatures.
data Entry = Entry
  { word  :: String
  , count :: !Int
  } deriving (Show, Eq)

newtype Freq a = Freq (Map.Map a Int)

(-->) :: Bool -> Bool -> Bool
a --> b = not a || b

infixr 1 -->

tokenize :: String -> [String]
tokenize = words . map normalize
  where
    normalize c
      | isAlpha c = toLower c
      | otherwise = ' '

count' :: Ord a => [a] -> Map.Map a Int
count' xs = foldr (\x m -> Map.insertWith (+) x 1 m) Map.empty xs

render :: Entry -> String
render (Entry w n) = w ++ ": " ++ show n ++ ['\t', '\n', '\'']

main :: IO ()
main = do
  let text = "the quick brown fox\tjumps over the lazy dog"
      freqs = count' (tokenize text)
      top = take 3 $ sortBy (\(_, a) (_, b) -> compare b a) (Map.toList freqs)
  mapM_ (putStrLn . render . uncurry Entry) top
  print (0x1F + 3.5e2 :: Double, True --> False)
