---- MODULE SymLiveMissedNone ----
\* SymLiveMissed without SYMMETRY: the violation TLC finds when it explores
\* all three states.
EXTENDS TLC
CONSTANTS a, b, c
VARIABLE x
S == {a, b, c}
Init == x = a
Next == x' \in S \ {x}
Spec == Init /\ [][Next]_x /\ WF_x(Next)
InfOftenA == []<>(x = a)
====
