% Family relations and list utilities in Prolog.
:- module(family, [ancestor/2, sum_list/2]).
:- use_module(library(lists)).

/* Facts: parent(Parent, Child). */
parent(tom, bob).
parent(bob, ann).
parent(bob, pat).
parent(pat, 'Jim Smith').

male(tom).
male(bob).

father(X, Y) :- parent(X, Y), male(X).

ancestor(X, Y) :- parent(X, Y).
ancestor(X, Y) :-
    parent(X, Z),
    ancestor(Z, Y).

% Accumulator style sum.
total([], Acc, Acc).
total([H|T], Acc, Sum) :-
    Acc1 is Acc + H,
    total(T, Acc1, Sum).

max_of(X, Y, Z) :- ( X >= Y -> Z = X ; Z = Y ).

classify(N, Kind) :-
    (   N =:= 0 -> Kind = zero
    ;   N < 0 -> Kind = negative
    ;   Kind = positive
    ).

not_parent(X) :- \+ parent(X, _), !.

greeting --> [hello], name.
name --> [world].

main :-
    findall(C, parent(bob, C), Children),
    length(Children, N),
    format("Bob has ~d children: ~w~n", [N, Children]),
    total([1, 2, 3.5, 0x1F], 0, Sum),
    Code = 0'a,
    format('sum=~w code=~w\n', [Sum, Code]),
    halt.

:- initialization(main).
