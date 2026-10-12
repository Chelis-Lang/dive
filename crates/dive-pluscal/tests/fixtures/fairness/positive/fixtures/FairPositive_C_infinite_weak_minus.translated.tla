---- MODULE FairPositive_C_infinite_weak_minus ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; fair process (P \in{1,2}) variables x=0; { L: - while (TRUE) { x:=(x+1)%2; }; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "a5f8cc69" /\ chksum(tla) = "7437e2dc")
VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self] = (x[self]+1)%2]
           /\ pc' = [pc EXCEPT ![self] = "L"]
           /\ g' = g

P(self) == L(self)

Next == (\E self \in {1,2}: P(self))

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in {1,2} : WF_vars((pc[self] # "L") /\ P(self))

\* END TRANSLATION 
====
