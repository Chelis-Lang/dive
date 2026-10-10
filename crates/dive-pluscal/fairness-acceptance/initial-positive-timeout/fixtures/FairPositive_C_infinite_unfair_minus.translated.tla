---- MODULE FairPositive_C_infinite_unfair_minus ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0;  process (P \in{1,2}) variables x=0; { L: - while (TRUE) { x:=(x+1)%2; }; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "58804c80" /\ chksum(tla) = "af0dfdb8")
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

Spec == Init /\ [][Next]_vars

\* END TRANSLATION 
====
