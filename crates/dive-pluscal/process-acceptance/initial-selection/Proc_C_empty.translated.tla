---- MODULE Proc_C_empty ----
EXTENDS Naturals,TLC
(* --algorithm A { process (P \in{}) { L: skip; E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "cb78f35f" /\ chksum(tla) = "15d18c82")
VARIABLE pc

vars == << pc >>

ProcSet == ({})

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

Next == (\E self \in {}: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
