---- MODULE ValueOverride ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
FalseAction == x' = x
Step == x' = x
Next == FalseAction \/ FalseAction \/ Step
====
