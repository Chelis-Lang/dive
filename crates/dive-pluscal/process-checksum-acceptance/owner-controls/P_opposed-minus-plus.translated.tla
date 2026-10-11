---- MODULE P_opposed_minus_plus ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; fair+ process P = 1 begin L:-g:=1; E:skip; end process; fair+ process Q = 2 begin M:+g:=2; F:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "572f6f4e" /\ chksum(tla) = "79c87574")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 2 -> "M"]

L == /\ pc[1] = "L"
     /\ g' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ g' = g

P == L \/ E

M == /\ pc[2] = "M"
     /\ g' = 2
     /\ pc' = [pc EXCEPT ![2] = "F"]

F == /\ pc[2] = "F"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ g' = g

Q == M \/ F

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ SF_vars((pc[1] # "L") /\ P)
        /\ SF_vars(Q)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
