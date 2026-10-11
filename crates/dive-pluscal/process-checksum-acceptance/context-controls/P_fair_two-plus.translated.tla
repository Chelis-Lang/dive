---- MODULE P_fair_two_plus ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; fair+ process P = 1 begin L:+g:=1; M:+g:=2; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "de4da539")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ g' = 1
     /\ pc' = [pc EXCEPT ![1] = "M"]

M == /\ pc[1] = "M"
     /\ g' = 2
     /\ pc' = [pc EXCEPT ![1] = "E"]

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ g' = g

P == L \/ M \/ E

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)
        /\ SF_vars(P)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
