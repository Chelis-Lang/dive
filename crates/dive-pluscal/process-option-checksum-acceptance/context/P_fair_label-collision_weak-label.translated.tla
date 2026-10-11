---- MODULE P_fair_label_collision_weak_label ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; process P = 1 begin Lbl_1:g:=1; g:=2; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "7b094bbf")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "Lbl_1"]

Lbl_1 == /\ pc[1] = "Lbl_1"
         /\ g' = 1
         /\ pc' = [pc EXCEPT ![1] = "Lbl_2"]

Lbl_2 == /\ pc[1] = "Lbl_2"
         /\ g' = 2
         /\ pc' = [pc EXCEPT ![1] = "Done"]

P == Lbl_1 \/ Lbl_2

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)
        /\ WF_vars(P)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
