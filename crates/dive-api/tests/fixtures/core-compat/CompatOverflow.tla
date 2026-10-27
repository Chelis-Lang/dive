---- MODULE CompatOverflow ----
\* TLC's integers are 32-bit: the second step overflows.
EXTENDS Naturals
VARIABLE x
Init == x \in {2147483645, 2147483646}
Next == x' = x + 1
====
