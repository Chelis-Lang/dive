---- MODULE RefSubstDouble ----
EXTENDS Naturals
VARIABLE x
I == INSTANCE In6 WITH v <- x
Init == x = 0
Next == UNCHANGED x
Inv == I!Double = 2 * x
====
