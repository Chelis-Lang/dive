---- MODULE P_two_minus ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables g=0; fair+ process P = 1 begin L:-g:=1; M:-g:=2; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "49aadd83" /\ chksum(tla) = "bca5f879")
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
        /\ SF_vars((pc[1] \notin {"L", "M"}) /\ P)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
