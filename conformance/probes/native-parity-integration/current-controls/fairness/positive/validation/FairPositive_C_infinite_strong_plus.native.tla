---- MODULE FairPositive_C_infinite_strong_plus ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0; fair + process (P \in{1,2}) variables x=0; { L: + while (TRUE) { x:=(x+1)%2; }; } } *)

VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({ 1 , 2 })

Init == /\ g = 0
        /\ x = [self \in { 1 , 2 } |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self] = ( x[self] + 1 ) % 2]
           /\ pc' = [pc EXCEPT ![self] = "L"]
           /\ g' = g

P(self) == L(self)

Next == (\E self \in { 1 , 2 }: P(self))

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in {1,2} : SF_vars(P(self))


====
