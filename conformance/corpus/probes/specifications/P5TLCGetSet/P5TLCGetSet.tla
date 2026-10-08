---- MODULE P5TLCGetSet ----
\* Probe: TLC-module state. TLCGet("level") is the length of the behaviour
\* prefix (1 for initial states), so Inv fails at x = 3. TLCSet/TLCGet(1) keep
\* a per-worker register: ASSUME sets it, and Init reads it back.
EXTENDS Naturals, TLC
ASSUME TLCSet(1, 42)
VARIABLE x
Init == x = 0 /\ TLCGet(1) = 42
Next == x' = (x + 1) % 10
Inv == TLCGet("level") < 4
====
