---- MODULE C_labels ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-label, -labelRoot Src)
(* --algorithm A {variables g=0; process(P=1){g:=1;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "4ffa8d33" /\ chksum(tla) = "8cdc624e")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "Src1"]

Src1 == /\ pc[1] = "Src1"
        /\ g' = 1
        /\ pc' = [pc EXCEPT ![1] = "Done"]

P == Src1

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
