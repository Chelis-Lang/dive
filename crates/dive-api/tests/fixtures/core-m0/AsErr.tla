---- MODULE AsErr ----
\* An ASSUME that fails to evaluate (TLC 2105, exit 10).
EXTENDS Naturals
CONSTANT N
ASSUME N \div 0 > 3
VARIABLE x
Init == x = 0
Next == x' = x
====
