---- MODULE Multi_C_ids_flag_wf ----
EXTENDS Naturals,TLC
(* --algorithm A { variable g=0;  process (P =1) variable x=0; { L: x:=1; E: g:=x; }  process (Q =2) variable y=0; { M: y:=2; F: g:=y; } } *)

VARIABLES pc, g, x, y

vars == << pc, g, x, y >>

ProcSet == {1} \cup {2}

Init == /\ g = 0
        /\ x = 0
        /\ y = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L" [] self = 2 -> "M"]

L == /\ pc[1] = "L"
     /\ x' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED << g, y >>

E == /\ pc[1] = "E"
     /\ g' = x
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, y >>

P == L \/ E

M == /\ pc[2] = "M"
     /\ y' = 2
     /\ pc' = [pc EXCEPT ![2] = "F"]
     /\ UNCHANGED << g, x >>

F == /\ pc[2] = "F"
     /\ g' = y
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, y >>

Q == M \/ F

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(P)
        /\ WF_vars(Q)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
