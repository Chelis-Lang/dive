---- MODULE P_two_constant ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P = 1 begin L:g:=g+1; E:skip; end process; process Q = 2 begin M:g:=g+1; F:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "25f391a3" /\ chksum(tla) = "fab2ad86")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1} \cup {2}

Init == /\ g = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L" [] self = 2 -> "M"]

L == /\ pc[1] = "L"
     /\ g' = g + 1
     /\ pc' = [pc EXCEPT ![1] = "E"]

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ g' = g

P == L \/ E

M == /\ pc[2] = "M"
     /\ g' = g + 1
     /\ pc' = [pc EXCEPT ![2] = "F"]

F == /\ pc[2] = "F"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ g' = g

Q == M \/ F

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")
\* END TRANSLATION 
====
