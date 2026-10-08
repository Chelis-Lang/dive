---- MODULE SelExceptSub ----
f == [x \in {1} |-> x]
F == [f EXCEPT ![1] = 2]
G == F!2
====
