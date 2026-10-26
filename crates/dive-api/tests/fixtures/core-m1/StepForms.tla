---- MODULE StepForms ----
\* [A]_v and <<A>>_v over non-variable subscripts, nested in the action,
\* and as the boolean forms the all-assigned shortcut evaluates.
EXTENDS Naturals
VARIABLES x, y
Init == x = 0 /\ y = 0
A == x' \in {x, (x + 1) % 4} /\ y' = y
Next == \/ <<A>>_(x + y)
        \/ [y < 2 /\ y' = y + 1 /\ x' = x]_<<x, y>>
        \/ (x' = x /\ y' = y /\ [FALSE]_y)
        \/ (x' = x /\ y' = 0 /\ <<TRUE>>_<<x, y>>)
====
