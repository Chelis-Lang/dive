---- MODULE ConfigError ----
EXTENDS Naturals
CONSTANT N
VARIABLE x
Init == x = N
Next == UNCHANGED x
====
