---- MODULE OpaqueTemporal ----
VARIABLE x
Init == x = FALSE
Next == x' = ~x
T(p) == []<>p
H(F(_),p) == F(p)
P == H(T,x)
====
