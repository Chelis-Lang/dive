---- MODULE FullIntIntervalInit ----
EXTENDS Integers
VARIABLE x
Init == x \in (-2147483647-1)..2147483647
Next == x' = x
Inv == x # x
====
