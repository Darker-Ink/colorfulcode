# Data pipeline helpers
/* Block comment
   spanning lines */
require! { fs, path }
{ map, filter, sum } = require 'prelude-ls'

MAX_SIZE = 1024

class Reader extends Base
  (@file, @encoding = 'utf8') ->
    super!

  read: ->
    fs.read-file-sync @file, @encoding

  lines: ~>
    @read!.split \\n

parse = (text) ->
  text
  |> (.split '\n')
  |> filter (isnt '')
  |> map (line) -> line.trim!

total = sum [1 to 10]
square = (x) -> x * x
greet = !-> console.log "Hello #{it}!\n"

result = switch
  | total > MAX_SIZE => \large
  | otherwise        => \small

pattern = /^\d+$/g
config =
  verbose: yes
  ratio: 1.5e2
console.log result, square 4, 0xFF
