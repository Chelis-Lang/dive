---- MODULE SimView ----
EXTENDS Naturals,TLC
VARIABLE x
ASSUME PrintT("assumption-must-not-run")
Init == x = Print("init-must-not-run", 0)
Next == x' = Print("step-must-not-run", x + 1)
V == x
====
