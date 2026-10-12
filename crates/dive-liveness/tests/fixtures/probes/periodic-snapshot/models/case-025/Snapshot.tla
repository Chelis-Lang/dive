---- MODULE Snapshot ----
EXTENDS Integers,TLC
CONSTANT Token
VARIABLE x
Init == x=0
Next == x'=1-x
Hidden == PrintT("SUBSTITUTION_MARKER")
Hook == TRUE
====
