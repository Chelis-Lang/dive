---- MODULE FairPositive_P_two_blocks ----
EXTENDS Naturals,TLC
(* --algorithm A variable g=0; fair process P=1 begin L: g:=1; end process; fair process Q=2 begin M: g:=2; end process; end algorithm; *)

VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1} \cup {2}

Init == /\ g = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L" [] self = 2 -> "M"]

L == /\ pc[1] = "L"
     /\ g' = 1
     /\ pc' = [pc EXCEPT ![1] = "Done"]

P == L

M == /\ pc[2] = "M"
     /\ g' = 2
     /\ pc' = [pc EXCEPT ![2] = "Done"]

Q == M

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(P)
        /\ WF_vars(Q)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
