---- MODULE PhaseDualErrorValid ----
EXTENDS Integers, TLC, Sequences
VARIABLE x
Init == x = 0
Next == x' \in {0,1,2}
Live == []<>(<<Head(<<>>) = 0>>_(1 \div x))
====
