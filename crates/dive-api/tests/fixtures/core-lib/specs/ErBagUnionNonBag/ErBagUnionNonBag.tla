---- MODULE ErBagUnionNonBag ----
\* BagUnion of a set holding a non-bag: TLC 2177, at x = 2.
EXTENDS Naturals, Bags
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == BagCardinality(BagUnion(IF x = 2 THEN {<<1>>, 2} ELSE {<<1>>, <<2>>})) > 0
====
