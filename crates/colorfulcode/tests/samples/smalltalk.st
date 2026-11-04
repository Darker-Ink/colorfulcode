"A simple bank account class"
Object subclass: #Account
    instanceVariableNames: 'balance owner'
    classVariableNames: 'Rate'
    package: 'Bank-Core'.

Account class >> owner: aString [
    ^ self new setOwner: aString
]

Account >> setOwner: aString [
    owner := aString.
    balance := 0
]

Account >> deposit: amount [
    amount <= 0 ifTrue: [ ^ self error: 'Invalid amount' ].
    balance := balance + amount
]

Account >> withdraw: amount [
    | fee |
    fee := amount * 1/100.
    (balance - amount - fee) < 0
        ifTrue: [ ^ InsufficientFunds signal: 'not enough money' ]
        ifFalse: [ balance := balance - amount - fee ]
]

Account >> printOn: aStream [
    aStream nextPutAll: owner; nextPut: $:; print: balance
]

| acct total squares |
acct := Account owner: 'Alice'.
acct deposit: 100; withdraw: 16r1F.
total := #(1 2 3 4) inject: 0 into: [:sum :each | sum + each].
squares := (1 to: 10) collect: [:x | x * x].
Transcript show: acct printString; cr.
#(#foo #bar:baz: #+ #'hello world' $a 2r1010 3.14e2 nil true) do: [:e | e yourself].
thisContext printString displayNl
