---- MODULE SimView ----
EXTENDS Naturals,TLC
VARIABLE x
ASSUME PrintT("assumption")
Init == x = Print("init", 0)
Next == x' = Print("step", x + 1)
V == x % 2
====
