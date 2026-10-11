---- MODULE P_fair_auto_owners_weak_label ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; process P = 1 begin g:=1; g:=2; end process; process Q \in {2,3} begin g:=1; g:=2; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "6ff2a2ab")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1} \cup ({2,3})

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "Lbl_1"
                                        [] self \in {2,3} -> "Lbl_3"]

Lbl_1 == /\ pc[1] = "Lbl_1"
         /\ g' = 1
         /\ pc' = [pc EXCEPT ![1] = "Lbl_2"]

Lbl_2 == /\ pc[1] = "Lbl_2"
         /\ g' = 2
         /\ pc' = [pc EXCEPT ![1] = "Done"]

P == Lbl_1 \/ Lbl_2

Lbl_3(self) == /\ pc[self] = "Lbl_3"
               /\ g' = 1
               /\ pc' = [pc EXCEPT ![self] = "Lbl_4"]

Lbl_4(self) == /\ pc[self] = "Lbl_4"
               /\ g' = 2
               /\ pc' = [pc EXCEPT ![self] = "Done"]

Q(self) == Lbl_3(self) \/ Lbl_4(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ (\E self \in {2,3}: Q(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)
        /\ WF_vars(P)
        /\ \A self \in {2,3} : WF_vars(Q(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
