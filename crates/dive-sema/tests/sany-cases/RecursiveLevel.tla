---- MODULE RecursiveLevel ----
VARIABLE x
RECURSIVE G(_)
G(n) == IF n = 0 THEN x ELSE G(0)
H == G(1)'
====
