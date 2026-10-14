---- MODULE EmptyExists ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Step == x' = x
Next == (\E k \in {}: FALSE) \/ Step
====
