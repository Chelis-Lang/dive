---- MODULE SharedFalse ----
VARIABLE x
Init == x = 0
FalseAction == FALSE
Step == x' = x
Next == FalseAction \/ FalseAction \/ Step
====
