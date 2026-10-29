---- MODULE AssumeError ----
EXTENDS Naturals
CONSTANT N
ASSUME N \div 0 = 0
VARIABLE x
Init == x = 0
Next == UNCHANGED x
====
