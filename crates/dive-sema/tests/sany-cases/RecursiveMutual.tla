---- MODULE RecursiveMutual ----
EXTENDS Naturals
RECURSIVE A(_), B(_)
A(n) == IF n = 0 THEN 0 ELSE B(n - 1)
B(n) == IF n = 0 THEN 1 ELSE A(n - 1)
====
