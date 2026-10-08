---- MODULE VarsBare2 ----
VARIABLES y, x
Init ==
  /\ y = 10
  /\ x = FALSE
Next ==
  \/ /\ y = 10
     /\ y' = 11
     /\ x' = TRUE
  \/ /\ y = 11
     /\ y' = 12
     /\ x' = FALSE
  \/ /\ y = 12
     /\ y' = 13
     /\ x' = TRUE
====
