---- MODULE Counter ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 4 THEN x + 1 ELSE x
Bound == x <= 4
StrictBound == x < 4
EventuallyFour == <>(x = 4)
====
