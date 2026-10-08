---- MODULE NxIfCase ----
\* Probe: IF and CASE in an action choose the branch to enumerate.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == IF x < 2
          THEN x' \in {x + 1, x + 2}
          ELSE CASE x = 2 -> x' = 10
                 [] x = 3 -> x' \in {11, 12}
                 [] OTHER -> FALSE
====
