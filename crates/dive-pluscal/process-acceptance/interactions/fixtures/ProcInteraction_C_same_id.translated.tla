---- MODULE ProcInteraction_C_same_id ----
EXTENDS Naturals,TLC
(* --algorithm A { process (P \in{1,2}) { L: skip; E: skip; }process (Q =1) { F: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "a50a8a5b" /\ chksum(tla) = "d94d9f37")
VARIABLE pc

vars == << pc >>

ProcSet == ({1,2}) \cup {1}

Init == /\ pc = [self \in ProcSet |-> CASE self \in {1,2} -> "L"
                                        [] self = 1 -> "F"]

L(self) == /\ pc[self] = "L"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "E"]

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]

P(self) == L(self) \/ E(self)

F == /\ pc[1] = "F"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]

Q == F

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Q
           \/ (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
