---- MODULE ErDeepRecursion ----
EXTENDS Integers, Sequences, FiniteSets, TLC
RECURSIVE Loop(_)
Loop(n) == Loop(n + 1)
ASSUME Loop(0) = 0
====
