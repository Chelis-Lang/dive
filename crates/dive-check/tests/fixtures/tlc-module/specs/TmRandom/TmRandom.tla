---- MODULE TmRandom ----
EXTENDS Naturals, TLC, Randomization
VARIABLE x, y
Init == x = RandomElement(1..1000) /\ y = 0
Next == /\ y < 4
        /\ \E z \in {RandomElement(1..1000), RandomElement(1..1000)}: x' = z
        /\ y' = y + 1
        /\ PrintT(<<x, y, RandomSubset(2, 1..10)>>)
Inv == PrintT(RandomElement({"a", "b", "c"}))
====
