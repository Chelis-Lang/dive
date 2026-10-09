---- MODULE SyTuple ----
CONSTANT a,b
VARIABLE x
Init == x = TRUE
Next == UNCHANGED x
Sym == {<<a,b>>,<<b,a>>}
====
