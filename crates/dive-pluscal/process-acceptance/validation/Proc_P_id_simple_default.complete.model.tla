---- MODULE Proc_P_id_simple_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 begin L: g:=1; E: skip; end process; end algorithm; *)

VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ g' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ g' = g

P == L \/ E

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
