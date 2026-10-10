---- MODULE Proc_C_fair_strong ----
EXTENDS Naturals,TLC
(* --algorithm A { fair + process (P \in{1,2}) { L: skip; E: skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "fdbade11" /\ chksum(tla) = "42f30f2b")
VARIABLE pc

vars == << pc >>

ProcSet == ({1,2})

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

Next == (\E self \in {1,2}: P(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in {1,2} : SF_vars(P(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
