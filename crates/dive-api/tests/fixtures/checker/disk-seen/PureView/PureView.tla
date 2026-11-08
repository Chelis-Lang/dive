---- MODULE PureView ----
EXTENDS Integers
VARIABLE x
Init == x = <<0, 0>>
Next == x[1] < 3 /\ x' = <<x[1] + 1, 1 - x[2]>>
View == x[1]
Inv == x[1] <= 3
====
