---- MODULE Fair ----
EXTENDS Integers
VARIABLES x, ack
vars == <<x, ack>>
Init == x = 0 /\ ack = FALSE
Next == x' = 0 /\ ack' = IF x' = 1 THEN ~ack ELSE ack
Spec == Init /\ [][Next]_vars /\ WF_vars(Next)
P == <>ack
====
