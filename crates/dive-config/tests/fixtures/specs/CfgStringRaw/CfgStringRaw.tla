---- MODULE CfgStringRaw ----
EXTENDS Naturals
CONSTANT N
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Inv == x < 2
ASSUME N = "a\\nb"
====
