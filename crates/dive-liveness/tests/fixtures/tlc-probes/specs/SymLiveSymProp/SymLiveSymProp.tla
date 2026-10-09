---- MODULE SymLiveSymProp ----
\* A symmetric spec with a symmetric property: the token alternates between
\* two processes under weak fairness, and every process holds it infinitely
\* often, \A p \in S : []<>(tok = p). It holds. Does TLC agree under
\* SYMMETRY?
EXTENDS TLC
CONSTANTS a, b
VARIABLE tok
S == {a, b}
Init == tok = a
Move == tok' = IF tok = a THEN b ELSE a
Spec == Init /\ [][Move]_tok /\ WF_tok(Move)
Sym == Permutations(S)
Fair == \A p \in S : []<>(tok = p)
====
