---- MODULE SuPrintRetry ----
EXTENDS SuPrintRetryBase, TLC
VARIABLE x
Init == x = 0
Next == x' = x
====
