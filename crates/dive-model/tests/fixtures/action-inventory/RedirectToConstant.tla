---- MODULE RedirectToConstant ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
FalseAction == x' = x
Replacement == FALSE
Step == x' = x
Next == FalseAction \/ FalseAction \/ Step
====
