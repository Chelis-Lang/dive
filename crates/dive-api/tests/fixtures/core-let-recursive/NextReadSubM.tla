---- MODULE NextReadSubM ----
EXTENDS Naturals
CONSTANT c
RECURSIVE R(_)
R(n) == IF n = 0 THEN c ELSE R(n-1)
====
