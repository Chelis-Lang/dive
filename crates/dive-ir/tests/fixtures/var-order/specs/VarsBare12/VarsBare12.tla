---- MODULE VarsBare12 ----
VARIABLES tmPrepared, rmState, votes, b_2, a1, state, Q, msgs, pc, z, y, x
Init ==
  /\ tmPrepared = 10
  /\ rmState = FALSE
  /\ votes = "s2_0"
  /\ b_2 = <<3, "t", FALSE>>
  /\ a1 = 50
  /\ state = FALSE
  /\ Q = "s6_0"
  /\ msgs = <<7, "t", FALSE>>
  /\ pc = 90
  /\ z = FALSE
  /\ y = "s10_0"
  /\ x = <<11, "t", FALSE>>
Next ==
  \/ /\ tmPrepared = 10
     /\ tmPrepared' = 11
     /\ rmState' = TRUE
     /\ votes' = "s2_1"
     /\ b_2' = <<4, "t", TRUE>>
     /\ a1' = 51
     /\ state' = TRUE
     /\ Q' = "s6_1"
     /\ msgs' = <<8, "t", TRUE>>
     /\ pc' = 91
     /\ z' = TRUE
     /\ y' = "s10_1"
     /\ x' = <<12, "t", TRUE>>
  \/ /\ tmPrepared = 11
     /\ tmPrepared' = 12
     /\ rmState' = FALSE
     /\ votes' = "s2_2"
     /\ b_2' = <<5, "t", FALSE>>
     /\ a1' = 52
     /\ state' = FALSE
     /\ Q' = "s6_2"
     /\ msgs' = <<9, "t", FALSE>>
     /\ pc' = 92
     /\ z' = FALSE
     /\ y' = "s10_2"
     /\ x' = <<13, "t", FALSE>>
  \/ /\ tmPrepared = 12
     /\ tmPrepared' = 13
     /\ rmState' = TRUE
     /\ votes' = "s2_3"
     /\ b_2' = <<6, "t", TRUE>>
     /\ a1' = 53
     /\ state' = TRUE
     /\ Q' = "s6_3"
     /\ msgs' = <<10, "t", TRUE>>
     /\ pc' = 93
     /\ z' = TRUE
     /\ y' = "s10_3"
     /\ x' = <<14, "t", TRUE>>
====
