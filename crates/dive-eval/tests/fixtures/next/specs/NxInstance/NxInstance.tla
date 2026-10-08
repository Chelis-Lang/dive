---- MODULE NxInstance ----
\* Probe: an action of an instance with a substituted variable assigns the
\* variable it is substituted by (getPrimedVar through the substitution).
EXTENDS Naturals
VARIABLES x, y
I == INSTANCE NxInstanceInner WITH v <- x
Init == x = 0 /\ y = 0
Next == I!Step /\ y' = y
====
