---- MODULE Proc_P_id_no_variables_default ----
EXTENDS Naturals, TLC
(* --algorithm A process P =1 begin L: skip; E: skip; end process; end algorithm; *)

VARIABLE pc

vars == << pc >>

ProcSet == {1}

Init == /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "E"]

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]

P == L \/ E

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
