---- MODULE P_fair_root_collision_weak_label ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; process P = 1 begin Auto1:g:=1; g:=2; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "1f98512b")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "Auto1"]

Auto1 == /\ pc[1] = "Auto1"
         /\ g' = 1
         /\ pc' = [pc EXCEPT ![1] = "Lbl_1"]

Lbl_1 == /\ pc[1] = "Lbl_1"
         /\ g' = 2
         /\ pc' = [pc EXCEPT ![1] = "Done"]

P == Auto1 \/ Lbl_1

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
