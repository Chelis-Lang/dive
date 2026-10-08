---- MODULE P9Random ----
\* Probe: RandomElement. TLC picks a pseudo-random element; without -seed the
\* choice differs between runs. The invariant fails in the initial state so
\* every tool prints the value it chose; comparing runs shows whether the
\* choice is seeded, replayable, or silently fixed.
EXTENDS Naturals, TLC
VARIABLE x
Init == x = RandomElement(1..1000000)
Next == UNCHANGED x
Inv == x < 0
====
