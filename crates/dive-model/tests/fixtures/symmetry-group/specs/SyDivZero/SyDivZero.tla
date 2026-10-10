---- MODULE SyDivZero ----
EXTENDS Integers
VARIABLE x
Init == x = TRUE
Next == UNCHANGED x
Sym == 1 \div 0
====
