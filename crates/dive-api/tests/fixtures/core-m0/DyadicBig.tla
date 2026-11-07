---- MODULE DyadicBig ----
\* DyadicRationals!Reduce of a numerator beyond 32 bits: TLC fails with a
\* ClassCastException, which the core refuses.
EXTENDS Naturals, DyadicRationals
VARIABLE x
Init == x = 0
Next == x' = x
Inv == Half([num |-> 2^40, den |-> 1]).den > 0
====
