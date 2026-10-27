---- MODULE ErBagCardNonBag ----
\* BagCardinality of a function with a zero count: TLC 2176, at x = 2.
EXTENDS Naturals, Bags
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == BagCardinality(IF x = 2 THEN <<1, 0>> ELSE <<1>>) > 0
====
