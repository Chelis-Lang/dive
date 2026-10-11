---- MODULE P_overlap ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; process P = 1 begin L:g:=g+1; E:skip; end process; process Q = 1 begin M:g:=1; F:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "f92d3a04" /\ chksum(tla) = "4f784c09")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1} \cup {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 1 -> "M"]

L == /\ pc[1] = "L"
     /\ g' = g+1
     /\ pc' = [pc EXCEPT ![1] = "E"]

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ g' = g

P == L \/ E

M == /\ pc[1] = "M"
     /\ g' = 1
     /\ pc' = [pc EXCEPT ![1] = "F"]

F == /\ pc[1] = "F"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ g' = g

Q == M \/ F

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
