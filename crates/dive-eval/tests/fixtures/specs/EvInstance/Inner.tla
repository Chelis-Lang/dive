---- MODULE Inner ----
EXTENDS Naturals
CONSTANT K
Double == K * 2
AddK(x) == x + K
Both == <<Double, AddK(1)>>
====
