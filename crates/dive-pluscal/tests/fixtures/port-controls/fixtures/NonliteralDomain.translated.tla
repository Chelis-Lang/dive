---- MODULE NonliteralDomain ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A process P \in S begin L: skip; end process; end algorithm
*)
\* BEGIN TRANSLATION (chksum(pcal) = "e0f3e97a" /\ chksum(tla) = "e763b167")
VARIABLE pc

vars == << pc >>

ProcSet == (S)

Init == /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]

P(self) == L(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in S: P(self))
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
