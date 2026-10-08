---- MODULE VarsStd1 ----
EXTENDS Integers, Sequences, FiniteSets, TLC, Bags
VARIABLES x
Init ==
  /\ x = 10
Next ==
  \/ /\ x = 10
     /\ x' = 11
   
  \/ /\ x = 11
     /\ x' = 12
   
  \/ /\ x = 12
     /\ x' = 13
   
====
