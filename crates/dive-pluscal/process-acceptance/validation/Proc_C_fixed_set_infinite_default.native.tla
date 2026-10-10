---- MODULE Proc_C_fixed_set_infinite_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables g=0; process (P \in{1,2}) variables x=0;  { L: while (TRUE) { x:=(x+1)%2; }; } } *)

VARIABLES g, x

vars == << g, x >>

ProcSet == ({ 1 , 2 })

Init == /\ g = 0
        /\ x = [self \in { 1 , 2 } |-> 0]

P(self) == /\ x' = [x EXCEPT ![self] = ( x[self] + 1 ) % 2]
           /\ g' = g

Next == (\E self \in { 1 , 2 }: P(self))

Spec == Init /\ [][Next]_vars


====
