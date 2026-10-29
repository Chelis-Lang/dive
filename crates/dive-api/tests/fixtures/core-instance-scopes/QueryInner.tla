---- MODULE QueryInner ----
EXTENDS Naturals
CONSTANT limit
VARIABLE x
Can == ENABLED(x' \in 0..limit /\ x' = x + 1)
====
