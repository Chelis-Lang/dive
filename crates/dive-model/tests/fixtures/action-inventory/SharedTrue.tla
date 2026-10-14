---- MODULE SharedTrue ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Always == TRUE
Step == x' = x
Next == Always \/ Always \/ Step
====
