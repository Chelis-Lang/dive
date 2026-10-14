---- MODULE ConstantComparison ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Never == (1 = 2)
Step == x' = x
Next == Never \/ Never \/ Step
====
