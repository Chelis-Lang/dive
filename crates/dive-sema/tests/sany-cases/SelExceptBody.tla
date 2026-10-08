---- MODULE SelExceptBody ----
F == [f EXCEPT ![1] = 2]
f == [x \in {1} |-> x]
G == 1
====
