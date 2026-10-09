---- MODULE SyNonBiject ----
CONSTANT a,b
VARIABLE x
Init == x = a
Next == x' \in {a,b}
Bad == {[p \in {a,b} |-> a], [p \in {a,b} |-> p]}
Sym == Bad
Inv == x = a
====
