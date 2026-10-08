---- MODULE VarsBare11 ----
VARIABLES rmState, votes, b_2, a1, state, Q, msgs, pc, z, y, x
Init ==
  /\ rmState = 10
  /\ votes = FALSE
  /\ b_2 = "s2_0"
  /\ a1 = <<3, "t", FALSE>>
  /\ state = 50
  /\ Q = FALSE
  /\ msgs = "s6_0"
  /\ pc = <<7, "t", FALSE>>
  /\ z = 90
  /\ y = FALSE
  /\ x = "s10_0"
Next ==
  \/ /\ rmState = 10
     /\ rmState' = 11
     /\ votes' = TRUE
     /\ b_2' = "s2_1"
     /\ a1' = <<4, "t", TRUE>>
     /\ state' = 51
     /\ Q' = TRUE
     /\ msgs' = "s6_1"
     /\ pc' = <<8, "t", TRUE>>
     /\ z' = 91
     /\ y' = TRUE
     /\ x' = "s10_1"
  \/ /\ rmState = 11
     /\ rmState' = 12
     /\ votes' = FALSE
     /\ b_2' = "s2_2"
     /\ a1' = <<5, "t", FALSE>>
     /\ state' = 52
     /\ Q' = FALSE
     /\ msgs' = "s6_2"
     /\ pc' = <<9, "t", FALSE>>
     /\ z' = 92
     /\ y' = FALSE
     /\ x' = "s10_2"
  \/ /\ rmState = 12
     /\ rmState' = 13
     /\ votes' = TRUE
     /\ b_2' = "s2_3"
     /\ a1' = <<6, "t", TRUE>>
     /\ state' = 53
     /\ Q' = TRUE
     /\ msgs' = "s6_3"
     /\ pc' = <<10, "t", TRUE>>
     /\ z' = 93
     /\ y' = TRUE
     /\ x' = "s10_3"
====
