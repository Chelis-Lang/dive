---- MODULE StartupRoleSplitBeforeFalseAssume ----
EXTENDS Naturals
VARIABLE x
ASSUME FALSE
Init == x = 0
Next == x' = 1 - x
Bad == IF x = 0 THEN 1 \div 0 = 0 ELSE TRUE
P == (x = 0) /\ (([]<>Bad) => TRUE)
====
