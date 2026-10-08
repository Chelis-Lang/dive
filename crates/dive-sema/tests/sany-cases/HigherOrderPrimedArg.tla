---- MODULE HigherOrderPrimedArg ----
VARIABLE x
F(G(_), b) == G(b)
H(a) == a'
K == F(H, x')
====
