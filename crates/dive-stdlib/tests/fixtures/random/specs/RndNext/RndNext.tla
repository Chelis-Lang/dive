---- MODULE RndNext ----
\* TLC reseeds the worker's generator to fingerprint(state) XOR seed when it
\* starts expanding a state, and keeps drawing from it across all actions.
EXTENDS Naturals, TLC, Randomization
VARIABLE x
Init == x = 0
Next == \/ /\ x < 3
           /\ PrintT(<<"a", x, RandomElement(1..1000000), RandomSubset(2, 1..10), RandomElement([1..2 -> 1..3])>>)
           /\ x' = x + 1
        \/ /\ x < 3
           /\ PrintT(<<"b", x, RandomSetOfSubsets(2, 1, 1..3), RandomElement({10, 20, 30})>>)
           /\ x' = x + 2
        \/ /\ x >= 3
           /\ PrintT(<<"c", x, RandomElement(1..1000000)>>)
           /\ UNCHANGED x
====
