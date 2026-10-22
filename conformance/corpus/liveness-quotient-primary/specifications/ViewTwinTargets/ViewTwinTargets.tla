---- MODULE ViewTwinTargets ----
\* Primary control: two concrete successors share an already-expanded VIEW
\* class, but differ on A. Reverse only changes their generation order.
EXTENDS Integers
CONSTANT Reverse
VARIABLES x, y
vars == <<x, y>>
Init == x = 0 /\ y = 0
Jump(v) == x' = 1 - x /\ y' = v
Next == IF Reverse THEN Jump(1) \/ Jump(0) ELSE Jump(0) \/ Jump(1)
A == x = 1 /\ Jump(1)
Spec == Init /\ [][Next]_vars /\ WF_vars(Next) /\ SF_vars(A)
Bad == []<>(x = 9)
View == x
====
