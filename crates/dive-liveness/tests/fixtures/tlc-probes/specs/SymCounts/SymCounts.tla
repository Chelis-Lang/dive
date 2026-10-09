---- MODULE SymCounts ----
\* Three interchangeable processes, each with a counter 0..2; a step
\* increments one counter. SYMMETRY Permutations(Procs). How many distinct
\* and generated states does TLC report? There are 10 orbits; SymCountsNone
\* has 27 states.
EXTENDS Naturals, TLC
CONSTANT Procs
VARIABLE c
Init == c = [p \in Procs |-> 0]
Next == \E p \in Procs : c[p] < 2 /\ c' = [c EXCEPT ![p] = @ + 1]
Perms == Permutations(Procs)
====
