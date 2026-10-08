---- MODULE LabelInExcept ----
f == [x \in {1} |-> x]
F == [f EXCEPT ![1] = lab :: 2]
====
