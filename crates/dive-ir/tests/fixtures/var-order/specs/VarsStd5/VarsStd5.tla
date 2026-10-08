---- MODULE VarsStd5 ----
EXTENDS Integers, Sequences, FiniteSets, TLC, Bags
VARIABLES x, y, z, pc, msgs
Init ==
  /\ x = 10
  /\ y = FALSE
  /\ z = "s2_0"
  /\ pc = <<3, "t", FALSE>>
  /\ msgs = 50
Next ==
  \/ /\ x = 10
     /\ x' = 11
     /\ y' = TRUE
     /\ z' = "s2_1"
     /\ pc' = <<4, "t", TRUE>>
     /\ msgs' = 51
  \/ /\ x = 11
     /\ x' = 12
     /\ y' = FALSE
     /\ z' = "s2_2"
     /\ pc' = <<5, "t", FALSE>>
     /\ msgs' = 52
  \/ /\ x = 12
     /\ x' = 13
     /\ y' = TRUE
     /\ z' = "s2_3"
     /\ pc' = <<6, "t", TRUE>>
     /\ msgs' = 53
====
