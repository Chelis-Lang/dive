---- MODULE SuPrintOrder ----
EXTENDS Naturals, TLC
CONSTANT N
VARIABLE x
A == Print("A", N)
B == Print("B", A + 1)
C == B + A
D == Print("D", x)
E == PrintT(<<"E", C>>)
Init == x = C
Next == x' = x
Inv == x = E[2]
====
