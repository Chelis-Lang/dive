---- MODULE BoxLetAction ----
EXTENDS Naturals
VARIABLE x
P == [](LET y == 1 IN x' = x + y)
====
