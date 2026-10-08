---- MODULE SuPrintRetryBase ----
EXTENDS Naturals, TLC
Retry == Print("retry", 1) = 1 /\ CHOOSE n \in {} : TRUE
====
