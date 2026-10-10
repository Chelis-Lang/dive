---- MODULE Proc_C_id_no_variables_default ----
EXTENDS Naturals, TLC
(* --algorithm A { process (P =1) {  L: skip; E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "14325899" /\ chksum(tla) = "104ec6a8")
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

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
