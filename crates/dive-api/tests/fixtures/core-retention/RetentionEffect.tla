---- MODULE RetentionEffect ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = Print(x,x+1)
====
