---- MODULE ParameterizedComparison ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Never(k) == k = 0
Step == x' = x
Next == \E k \in {1, 2}: Never(k) \/ Step
====
