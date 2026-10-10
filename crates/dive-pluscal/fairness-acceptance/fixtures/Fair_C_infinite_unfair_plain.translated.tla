---- MODULE Fair_C_infinite_unfair_plain ----
EXTENDS Naturals,TLC
(* --algorithm A { variables g=0;  process (P \in{1,2}) variables x=0; { L: while (TRUE) { x:=(x+1)%2; }; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "955633a1" /\ chksum(tla) = "b5cf94a8")
VARIABLES g, x

vars == << g, x >>

ProcSet == ({1,2})

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ x = [self \in {1,2} |-> 0]

P(self) == /\ x' = [x EXCEPT ![self] = (x[self]+1)%2]
           /\ g' = g

Next == (\E self \in {1,2}: P(self))

Spec == Init /\ [][Next]_vars

\* END TRANSLATION 
====
