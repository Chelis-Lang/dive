---- MODULE Proc_C_duplicate_ids ----
EXTENDS Naturals,TLC
(* --algorithm A { process (P \in{1,1}) { L: skip; E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "84691957" /\ chksum(tla) = "c733bf5f")
VARIABLE pc

vars == << pc >>

ProcSet == ({1,1})

Init == /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "E"]

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]

P(self) == L(self) \/ E(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in {1,1}: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
