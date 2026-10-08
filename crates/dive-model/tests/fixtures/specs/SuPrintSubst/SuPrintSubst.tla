---- MODULE SuPrintSubst ----
EXTENDS SuPrintSubstBase, TLC
VARIABLE x
D == Print("D", 7)
Init == x = K
Next == x' = x
====
