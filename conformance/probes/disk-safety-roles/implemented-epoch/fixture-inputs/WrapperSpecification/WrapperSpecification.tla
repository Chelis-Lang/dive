---- MODULE WrapperSpecification ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x < 3 /\ x' = x + 1
Wrap(I, N) == I /\ [][N]_x
Spec == Wrap(Init, Next)
====
