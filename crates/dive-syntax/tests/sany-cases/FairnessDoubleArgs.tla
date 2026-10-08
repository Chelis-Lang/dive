---- MODULE FairnessDoubleArgs ----
VARIABLE v
f(a) == a
x == WF_f(v)(v' = v)
====
