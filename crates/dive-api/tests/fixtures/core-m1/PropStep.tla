---- MODULE PropStep ----
\* An implied action violated on a step to a state already seen: y returns
\* to 0 only from x = 2, after both values of y were seen at x = 0.
EXTENDS Naturals
VARIABLES x, y
Init == x = 0 /\ y = 0
Next == \/ x < 2 /\ x' = x + 1 /\ y' = y
        \/ y' = 1 - y /\ x' = x
Prop == [][y' = 0 => x # 2 \/ y = 0]_<<x, y>>
Start == y = 0
====
