---- MODULE ActiveAlias ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Base(k) == x' = k
Alias(k) == Base(k)
Next == \E k \in {1, 2}: Alias(k)
====
