---- MODULE RoInstance ----
EXTENDS Naturals
VARIABLE z
I == INSTANCE RoInstanceInner WITH c <- z
Spec == I!Spec
====
