---- MODULE ErBagDiffNonFcn ----
\* A (-) operand that is not a function: TLC 2169, at x = 1.
EXTENDS Naturals, Bags
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Inv == BagCardinality(<<1>> (-) (IF x = 1 THEN 3 ELSE <<1>>)) >= 0
====
