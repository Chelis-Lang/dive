---- MODULE HigherTemporal ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
T(p) == []<>p
H(F(_),p) == F(p)
Apply2(G(_,_),p) == G(p,p)
P == Apply2(H,x)
====
