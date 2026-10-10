---- MODULE Proc_P_set_self_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P \in{1,2} variables x=self; begin L: g:=x; E: x:=self+1; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "ffdeaae4" /\ chksum(tla) = "a0c465c7")
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> self]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ g' = x[self]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ x' = x

E(self) == /\ pc[self] = "E"
           /\ x' = [x EXCEPT ![self] = self+1]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ g' = g

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
