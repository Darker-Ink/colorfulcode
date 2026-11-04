⍝ Dyalog APL samples
⎕IO←0
data←3 1 4 1 5 9 2 6 5 3 5
avg←{(+⌿⍵)÷≢⍵}
avg data
sorted←data[⍋data]
primes←{(2=+⌿0=⍵∘.|⍵)/⍵}⍳50
life←{⊃1 ⍵∨.∧3 4=+/,¯1 0 1∘.⊖¯1 0 1∘.⌽⊂⍵}
mat←3 3⍴⍳9
⍉mat
'Hello, ''APL'' world'
1.5E3 ¯2.25 0J1
fib←{⍵≤1:⍵ ⋄ (∇⍵-1)+∇⍵-2}
fib¨⍳10
unique←∪data
⍬≡⍳0
∇ r←Square x
  r←x×x
∇
:If 0<≢data
    ⎕←'non-empty'
:Else
    ⎕←'empty'
:EndIf
counts←{⍺,≢⍵}⌸data
∆total←+/data ⍝ sum
