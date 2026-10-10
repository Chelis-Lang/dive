---- MODULE Proc_C_fixed_id_default_global_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g; process (P =1) variables x=0;  { L: g:=x; E: skip; } } *)

CONSTANT defaultInitValue

VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == {1}

Init == /\ g = defaultInitValue
        /\ x = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ g' = x
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ x' = x

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
