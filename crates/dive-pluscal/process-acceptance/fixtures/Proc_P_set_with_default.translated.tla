---- MODULE Proc_P_set_with_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P \in{1,2} variables x=0; begin L: with k\in{0,1} do x:=k; end with; E: g:=x; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "be0b38ef" /\ chksum(tla) = "8ce74ebf")
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ \E k \in {0,1}:
                x' = [x EXCEPT ![self] = k]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ g' = g

E(self) == /\ pc[self] = "E"
           /\ g' = x[self]
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
