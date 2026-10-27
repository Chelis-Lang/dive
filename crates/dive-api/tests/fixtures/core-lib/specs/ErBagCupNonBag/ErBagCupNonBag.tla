---- MODULE ErBagCupNonBag ----
\* The first argument of (+) is not a bag: TLC 2169, at x = 1.
EXTENDS Naturals, Bags
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == BagCardinality((IF x = 1 THEN <<0>> ELSE <<1>>) (+) <<1>>) > 0
====
