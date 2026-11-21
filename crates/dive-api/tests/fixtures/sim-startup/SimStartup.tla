---- MODULE SimStartup ----
\* A pure constant definition used only as a value by Next (issue #1142):
\* Core evaluates it when it compiles its plan; the binder does not.
EXTENDS Naturals
VARIABLE x
Ring == CHOOSE r \in [1..4 -> 1..4] : \A i \in 1..4 : r[i] = (i % 4) + 1
Init == x = 1
Next == x' = Ring[x]
Inv == x \in 1..4
====
