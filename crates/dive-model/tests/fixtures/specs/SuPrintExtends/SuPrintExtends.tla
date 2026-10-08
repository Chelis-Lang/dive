---- MODULE SuPrintExtends ----
EXTENDS SuPrintExtendsBase, TLC
VARIABLE x
R == Print("R", 2)
Fails == Print("Fails", 1 \div 0)
Init == x = R
Next == x' = x
====
