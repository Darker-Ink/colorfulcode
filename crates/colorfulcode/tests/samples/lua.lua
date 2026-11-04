--[[ A tiny class
     implementation for Lua ]]
local Stack = {}
Stack.__index = Stack

local MAX_SIZE = 100

--- Creates a new stack.
function Stack.new(items)
  local self = setmetatable({}, Stack)
  self.items = items or {}
  return self
end

function Stack:push(value)
  if #self.items >= MAX_SIZE then
    error("stack overflow: " .. tostring(#self.items))
  end
  table.insert(self.items, value)
end

function Stack:pop()
  return table.remove(self.items)
end

local doc = [==[
Long string with ]] inside
]==]

local s = Stack.new({1, 2.5, 0xFF, 1e3})
s:push('it\'s "quoted"\n')
for i, v in ipairs(s.items) do
  print(i, v ~= nil and v or "nil")
end
--[==[ block
comment ]==]
local ok, err = pcall(function() return s:pop() end)
