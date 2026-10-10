---- MODULE SyWarnOverride ----
EXTENDS TLC
CONSTANT SpecSet, a
CfgSet == {a}
Sym == Permutations(CfgSet)
VARIABLE x
Init == x = TRUE
Next == UNCHANGED x
====
