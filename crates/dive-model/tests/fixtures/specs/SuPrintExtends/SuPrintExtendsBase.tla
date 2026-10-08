---- MODULE SuPrintExtendsBase ----
EXTENDS Naturals, TLC
Base == Print("Base", 1)
BaseFails == Print("BaseFails", CHOOSE n \in {} : TRUE)
====
