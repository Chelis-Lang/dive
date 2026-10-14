---- MODULE RuntimePrintConditional ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Next == x' = x + 1 /\ (IF x = 0 THEN PrintT(x) ELSE RandomElement({TRUE}))
Safe == x < 2
====
