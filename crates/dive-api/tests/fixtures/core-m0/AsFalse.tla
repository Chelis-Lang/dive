---- MODULE AsFalse ----
\* A false ASSUME ends the run before any state (TLC 2104, exit 10).
EXTENDS Naturals
CONSTANT N
ASSUME N > 3
VARIABLE x
Init == x = 0
Next == x' = x
====
