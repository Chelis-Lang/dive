---- MODULE Safety ----
\* An invariant ends the search before any temporal verdict.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x + 1
Inv == x < 5
P == []<>(x = 0)
====
