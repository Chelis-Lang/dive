---- MODULE EventuallyAlwaysTrueNotSyntacticZero ----
EXTENDS Naturals, TLC
VARIABLE x
Init == PrintT("INIT_SEEN") /\ x = 0
Next == x' = 1 - x
Bad == IF x = 0 THEN 1 \div 0 = 0 ELSE TRUE
P == <>[]TRUE
====
