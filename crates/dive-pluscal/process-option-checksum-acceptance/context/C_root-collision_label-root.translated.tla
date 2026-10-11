---- MODULE C_root_collision_label_root ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P=1){Auto1:g:=1; g:=2;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "606f6fc9" /\ chksum(tla) = "99142ade")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "Auto1"]

Auto1 == /\ pc[1] = "Auto1"
         /\ g' = 1
         /\ pc' = [pc EXCEPT ![1] = "Auto2"]

Auto2 == /\ pc[1] = "Auto2"
         /\ g' = 2
         /\ pc' = [pc EXCEPT ![1] = "Done"]

P == Auto1 \/ Auto2

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
