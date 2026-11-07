---- MODULE RedirectInvariant ----
\* `Op <- D` on the safety route: each use of Op evaluates D's body, so the
\* invariant compares two draws and can fail (TLC semantics).
EXTENDS Naturals, TLC
Op == 0
D == RandomElement(0..1)
VARIABLE x
Init == x = Op
Next == x' = Op
Inv == x = Op
====
