---- MODULE DirectScope ----
EXTENDS Naturals
CONSTANT N
VARIABLE pending, done
vars == <<pending, done>>
Init == pending = 1..N /\ done = FALSE
Schedule == pending' = {} /\ done' = TRUE
Next == Schedule
Spec == Init /\ [][Next]_vars /\ WF_vars(Schedule)
Good == <>done
====
