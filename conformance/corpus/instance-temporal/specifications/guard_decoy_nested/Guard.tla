---------------- MODULE Guard ----------------
VARIABLE y
A == /\ y = FALSE /\ y' = TRUE
Can == ENABLED A
================================================
