---- MODULE C_label_collision_label_root ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P=1){Lbl_1:g:=1; g:=2;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "b657987a" /\ chksum(tla) = "230bded8")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "Lbl_1"]

Lbl_1 == /\ pc[1] = "Lbl_1"
         /\ g' = 1
         /\ pc' = [pc EXCEPT ![1] = "Auto1"]

Auto1 == /\ pc[1] = "Auto1"
         /\ g' = 2
         /\ pc' = [pc EXCEPT ![1] = "Done"]

P == Lbl_1 \/ Auto1

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
