---- MODULE WideProductInit ----
EXTENDS Naturals
VARIABLE x
Init == x \in (1..50000) \X (1..50000)
Next == UNCHANGED x
Bad == x # x
====
