------------------------------- MODULE TwoPhase -------------------------------
(* Two phase commit, simplified *)
EXTENDS Naturals, Sequences, FiniteSets, TLC

CONSTANTS RM, MaxRetries   \* the resource managers
VARIABLES rmState, tmState, msgs

vars == <<rmState, tmState, msgs>>

Messages == [type : {"Prepared"}, rm : RM] \cup [type : {"Commit", "Abort"}]

TypeOK ==
    /\ rmState \in [RM -> {"working", "prepared", "committed", "aborted"}]
    /\ tmState \in {"init", "done"}
    /\ msgs \subseteq Messages

Init ==
    /\ rmState = [r \in RM |-> "working"]
    /\ tmState = "init"
    /\ msgs = {}

Prepare(r) ==
    /\ rmState[r] = "working"
    /\ rmState' = [rmState EXCEPT ![r] = "prepared"]
    /\ msgs' = msgs \cup {[type |-> "Prepared", rm |-> r]}
    /\ UNCHANGED tmState

Commit == /\ \A r \in RM : rmState[r] = "prepared"
          /\ tmState' = "done"
          /\ UNCHANGED <<rmState, msgs>>

Next == Commit \/ \E r \in RM : Prepare(r)

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

THEOREM Spec => []TypeOK

(* --algorithm counter
variables x = 0;
begin
  Loop: while x < 3 do
    x := x + 1;
  end while;
end algorithm; *)
=============================================================================
