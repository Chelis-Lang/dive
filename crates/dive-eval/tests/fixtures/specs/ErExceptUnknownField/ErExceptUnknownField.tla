---- MODULE ErExceptUnknownField ----
EXTENDS Integers, Sequences, FiniteSets, TLC
ASSUME LET r == [a |-> 1] IN [r EXCEPT !.b = 2] = r
====
