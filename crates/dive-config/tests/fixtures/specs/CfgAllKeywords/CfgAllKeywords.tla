---- MODULE CfgAllKeywords ----
EXTENDS Naturals, TLC
CONSTANTS N, S, R, B, E, M, T, U
VARIABLE x
Tdef == 7
Init == x = 0
Next == x' = (x + 1) % 3
Inv1 == x \in 0..2
Inv2 == N = 3
Inv3 == T = 7
Prop == []Inv1
Prop2 == [][x' /= x]_x
Con == x < 10
Con2 == TRUE
ACon == x' /= x
ACon2 == TRUE
V == x
Al == [x |-> x, s |-> S]
Post == TLCGet("distinct") = 3
Post2 == TLCGet("distinct") > 0 /\ U = "a\\nb"
ASSUME M = M /\ S = S /\ R = {"s", "t"} /\ B /\ E = {}
====
