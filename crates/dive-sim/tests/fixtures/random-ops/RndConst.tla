---- MODULE RndConst ----
EXTENDS Integers, TLC
C == RandomElement(1..1000000)
VARIABLE x
Init == x = C
Next == x' = C
Safe == x >= 0
View == x % 2
====
