---- MODULE BigTrace ----
\* A counterexample whose states hold integers outside the core's inline range.
EXTENDS Integers
VARIABLES x
Init == x = 2^62
Next == x' = x + 1
Inv == x < 2^62 + 2
====
