---- MODULE Inactive ----
VARIABLE x
Init == x = 0
Disabled == FALSE
Step == x' = x
Next == Disabled \/ Step
====
