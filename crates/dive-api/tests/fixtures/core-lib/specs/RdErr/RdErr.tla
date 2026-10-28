---- MODULE RdErr ----
\* A draw that fails during the search: RandomElement of the empty set.
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == x' = IF x = 3 THEN RandomElement({}) ELSE x + RandomElement({1})
====
