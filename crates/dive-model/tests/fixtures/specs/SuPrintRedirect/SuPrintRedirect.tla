---- MODULE SuPrintRedirect ----
EXTENDS SuPrintRedirectBase, TLC
VARIABLE x
Other == Print("Other", 5)
Init == x = Op
Next == x' = x
====
