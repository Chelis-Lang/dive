---- MODULE SymLiveMissed ----
\* A real liveness violation that symmetry hides. x moves to a different
\* model value at every step, under weak fairness. The behaviour a, b, c, b,
\* c, ... violates []<>(x = a). Under SYMMETRY all three states are one
\* class, so only a is explored. Does TLC report success? Compare
\* SymLiveMissedNone.
EXTENDS TLC
CONSTANTS a, b, c
VARIABLE x
S == {a, b, c}
Init == x = a
Next == x' \in S \ {x}
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Sym == Permutations(S)
InfOftenA == []<>(x = a)
====
