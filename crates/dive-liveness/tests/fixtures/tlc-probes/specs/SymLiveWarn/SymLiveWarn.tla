---- MODULE SymLiveWarn ----
\* Survey probe P4: x alternates between two symmetric model values under
\* weak fairness, so <>(x = b) holds. With SYMMETRY the two states are one
\* class. TLC prints its liveness-with-symmetry warning; what verdict and
\* counts does it give?
EXTENDS TLC
CONSTANTS a, b
VARIABLE x
S == {a, b}
Init == x = a
Next == x' = IF x = a THEN b ELSE a
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Sym == Permutations(S)
EventuallyB == <>(x = b)
====
