---- MODULE HigherOrderBad ----
F(G(_)) == G(1)
H(a, b) == a
X == F(H)
====
