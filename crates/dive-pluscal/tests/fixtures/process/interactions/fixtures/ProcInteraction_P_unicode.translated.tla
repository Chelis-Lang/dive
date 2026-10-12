---- MODULE ProcInteraction_P_unicode ----
EXTENDS Naturals,TLC
(* --algorithm A process P \in{1,2} begin \* λ🙂
	L: skip; E: skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "957c9a73" /\ chksum(tla) = "d03542f")
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

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
