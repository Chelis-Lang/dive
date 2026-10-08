---- MODULE SuPrintAssume ----
EXTENDS Naturals, TLC
CONSTANT N
P == Print("P", N)
ASSUME Print("assume", TRUE)
ASSUME P > 5
====
