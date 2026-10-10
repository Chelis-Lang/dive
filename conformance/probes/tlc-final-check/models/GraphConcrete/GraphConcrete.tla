---- MODULE GraphConcrete ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = <<1>>
Next == x' = [i \in 1..1 |-> i]
Live == []<>(x = <<2>>)
====
