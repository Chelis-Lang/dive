---- MODULE HigherOrderLevel ----
VARIABLE x
F(G(_)) == G(x)
H(a) == a'
K == F(H)
====
