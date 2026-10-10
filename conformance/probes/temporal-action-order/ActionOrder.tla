---- MODULE ActionOrder ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == x' \in {0, 1, 2}
Body == IF x' = 1 THEN 1 \div 0 = 0 ELSE x' = 2
FalseBody == FALSE
BadSub == 1 \div (x - x)
Sub == x
Runtime == <<Body>>_x
====
