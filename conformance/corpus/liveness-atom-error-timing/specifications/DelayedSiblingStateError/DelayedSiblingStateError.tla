---- MODULE DelayedSiblingStateError ----
EXTENDS Naturals
VARIABLE x

Init == x = 0
Next == IF x = 0 THEN x' \in {1, 2}
        ELSE IF x = 1 THEN x' = 3
        ELSE x' = x
Bad == IF x = 1 THEN 1 \div 0 = 0 ELSE TRUE
P == []<>Bad
====
