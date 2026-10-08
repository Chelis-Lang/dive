---- MODULE VarsBare5 ----
VARIABLES msgs, pc, z, y, x
Init ==
  /\ msgs = 10
  /\ pc = FALSE
  /\ z = "s2_0"
  /\ y = <<3, "t", FALSE>>
  /\ x = 50
Next ==
  \/ /\ msgs = 10
     /\ msgs' = 11
     /\ pc' = TRUE
     /\ z' = "s2_1"
     /\ y' = <<4, "t", TRUE>>
     /\ x' = 51
  \/ /\ msgs = 11
     /\ msgs' = 12
     /\ pc' = FALSE
     /\ z' = "s2_2"
     /\ y' = <<5, "t", FALSE>>
     /\ x' = 52
  \/ /\ msgs = 12
     /\ msgs' = 13
     /\ pc' = TRUE
     /\ z' = "s2_3"
     /\ y' = <<6, "t", TRUE>>
     /\ x' = 53
====
