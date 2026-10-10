---- MODULE Proc_P_id_sequential_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P =1 variables x=0; begin L: x:=1; g:=x; E: skip; end process; end algorithm; *)

VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == {1}

Init == /\ g = 0
        /\ x = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ x' = 1
     /\ g' = x'
     /\ pc' = [pc EXCEPT ![1] = "E"]

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, x >>

P == L \/ E

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
