---- MODULE C_corrected_mixed_blocks ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P = 1) {L:g:=g+1; E:skip;} process(Q \in {2,3}) variables z=0; {M:z:=1; F:g:=z;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "5929bf7a" /\ chksum(tla) = "16467ba4")
VARIABLES pc, g, z

vars == << pc, g, z >>

ProcSet == {1} \cup ({2,3})

Init == (* Global variables *)
        /\ g = 0
        (* Process Q *)
        /\ z = [self \in {2,3} |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self \in {2,3} -> "M"]

L == /\ pc[1] = "L"
     /\ g' = g+1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ z' = z

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, z >>

P == L \/ E

M(self) == /\ pc[self] = "M"
           /\ z' = [z EXCEPT ![self] = 1]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ g' = g

F(self) == /\ pc[self] = "F"
           /\ g' = z[self]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ z' = z

Q(self) == M(self) \/ F(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ (\E self \in {2,3}: Q(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
