---- MODULE FairPositive_C_infinite_weak_plus ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; fair process (P \in{1,2}) variables x=0; { L: + while (TRUE) { x:=(x+1)%2; }; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "6c1ac38" /\ chksum(tla) = "87424f26")
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
        /\ \A self \in {1,2} : WF_vars(P(self)) /\ SF_vars(L(self))

\* END TRANSLATION 
====
