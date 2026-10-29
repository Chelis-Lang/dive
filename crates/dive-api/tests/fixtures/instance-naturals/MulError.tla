---- MODULE MulError ----
EXTENDS Naturals
VARIABLE v
Left == v * (1 \div 0)
Right == (1 \div 0) * v
====
