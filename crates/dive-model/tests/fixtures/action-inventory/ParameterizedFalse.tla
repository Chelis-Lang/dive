---- MODULE ParameterizedFalse ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Never(k) == FALSE
Step == x' = x
Next == Never(1) \/ Never(2) \/ Step
====
