---- MODULE ImpliedActionSeen ----
\* Probe: action properties ([][A]_v) are checked on every generated step,
\* including steps into states already seen. The only violating step is
\* 1 -> 0, and 0 is the initial state. Expected: violation of Prop after
\* 3 generated and 2 distinct states, with a 3-state counterexample.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = 1 - x
Prop == [][x' = 1]_x
====
