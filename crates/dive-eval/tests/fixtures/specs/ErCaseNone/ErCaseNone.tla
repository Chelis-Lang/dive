---- MODULE ErCaseNone ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME CASE 1 = 2 -> 1 [] 2 = 3 -> 2
====
