---- MODULE Proc_P_set_record_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P \in{1,2} variables x=[a|->0]; begin L: x.a:=self; E: g:=x.a; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "a86c78a" /\ chksum(tla) = "c937e44c")
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> [a|->0]]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self].a = self]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ g' = g

E(self) == /\ pc[self] = "E"
           /\ g' = x[self].a
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ x' = x

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
