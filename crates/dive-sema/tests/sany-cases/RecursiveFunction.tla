---- MODULE RecursiveFunction ----
EXTENDS Naturals
RECURSIVE f
f[n \in Nat] == IF n = 0 THEN 0 ELSE f[n - 1]
====
