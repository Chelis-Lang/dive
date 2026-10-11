---- MODULE C_fair_missing_weak_label ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P \in {1,2}){g:=1; g:=2;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "60a72921")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "Lbl_1"]

Lbl_1(self) == /\ pc[self] = "Lbl_1"
               /\ g' = 1
               /\ pc' = [pc EXCEPT ![self] = "Lbl_2"]

Lbl_2(self) == /\ pc[self] = "Lbl_2"
               /\ g' = 2
               /\ pc' = [pc EXCEPT ![self] = "Done"]

P(self) == Lbl_1(self) \/ Lbl_2(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)
        /\ \A self \in {1,2} : WF_vars(P(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
