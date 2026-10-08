---- MODULE RecursiveInLet ----
EXTENDS Naturals
F(n) == LET RECURSIVE G(_)
            G(m) == IF m = 0 THEN 0 ELSE G(m - 1)
        IN G(n)
====
