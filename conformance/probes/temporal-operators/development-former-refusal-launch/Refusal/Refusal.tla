---- MODULE Refusal ----
EXTENDS TLC
VARIABLE x
Init == x = TRUE
Next == x' = ~x
F(a) == WF_x(a)
Fair == F(Next)
Effect == []Print(x,x)
Read == [](TLCGet("level") = 0)
T(p) == []p
Param == T(x)
H(G(_),p) == G(p)
Higher == H(T,x)
Bad == <>1
====
