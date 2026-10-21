---- MODULE LargeConstructorFails ----
EXTENDS Naturals
VARIABLE x
Init == x = TRUE
Next == x' = ~x
P == []({b : b \in 1..5000} = {})
====
