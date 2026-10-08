---- MODULE VarsStd2 ----
EXTENDS Integers, Sequences, FiniteSets, TLC, Bags
VARIABLES x, y
Init ==
  /\ x = 10
  /\ y = FALSE
Next ==
  \/ /\ x = 10
     /\ x' = 11
     /\ y' = TRUE
  \/ /\ x = 11
     /\ x' = 12
     /\ y' = FALSE
  \/ /\ x = 12
     /\ x' = 13
     /\ y' = TRUE
====
