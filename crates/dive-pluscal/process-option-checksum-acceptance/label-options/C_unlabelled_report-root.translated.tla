---- MODULE C_unlabelled_report_root ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P=1){g:=1; g:=2;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "9ffa0764" /\ chksum(tla) = "ee5b759")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "X_1"]

X_1 == /\ pc[1] = "X_1"
       /\ g' = 1
       /\ pc' = [pc EXCEPT ![1] = "X_2"]

X_2 == /\ pc[1] = "X_2"
       /\ g' = 2
       /\ pc' = [pc EXCEPT ![1] = "Done"]

P == X_1 \/ X_2

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
