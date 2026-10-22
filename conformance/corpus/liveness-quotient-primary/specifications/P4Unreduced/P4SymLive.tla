---- MODULE P4SymLive ----
\* Probe: symmetry reduction combined with liveness. x alternates between the
\* two model values forever under weak fairness, so <>(x = b) holds. Under
\* SYMMETRY the two states collapse into one class, and a checker that checks
\* liveness on the quotient graph can report a spurious violation (TLC warns
\* that this combination is unsound). An honest tool refuses or warns loudly.
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
