---- MODULE ErCaseNoneNested ----
EXTENDS Integers, Sequences, FiniteSets, TLC
Op(x) == CASE x = 1 -> 1
ASSUME Op(2) = 1
====
