---- MODULE VarsStd13 ----
EXTENDS Integers, Sequences, FiniteSets, TLC, Bags
VARIABLES x, y, z, pc, msgs, Q, state, a1, b_2, votes, rmState, tmPrepared, AaAa
Init ==
  /\ x = 10
  /\ y = FALSE
  /\ z = "s2_0"
  /\ pc = <<3, "t", FALSE>>
  /\ msgs = 50
  /\ Q = FALSE
  /\ state = "s6_0"
  /\ a1 = <<7, "t", FALSE>>
  /\ b_2 = 90
  /\ votes = FALSE
  /\ rmState = "s10_0"
  /\ tmPrepared = <<11, "t", FALSE>>
  /\ AaAa = 130
Next ==
  \/ /\ x = 10
     /\ x' = 11
     /\ y' = TRUE
     /\ z' = "s2_1"
     /\ pc' = <<4, "t", TRUE>>
     /\ msgs' = 51
     /\ Q' = TRUE
     /\ state' = "s6_1"
     /\ a1' = <<8, "t", TRUE>>
     /\ b_2' = 91
     /\ votes' = TRUE
     /\ rmState' = "s10_1"
     /\ tmPrepared' = <<12, "t", TRUE>>
     /\ AaAa' = 131
  \/ /\ x = 11
     /\ x' = 12
     /\ y' = FALSE
     /\ z' = "s2_2"
     /\ pc' = <<5, "t", FALSE>>
     /\ msgs' = 52
     /\ Q' = FALSE
     /\ state' = "s6_2"
     /\ a1' = <<9, "t", FALSE>>
     /\ b_2' = 92
     /\ votes' = FALSE
     /\ rmState' = "s10_2"
     /\ tmPrepared' = <<13, "t", FALSE>>
     /\ AaAa' = 132
  \/ /\ x = 12
     /\ x' = 13
     /\ y' = TRUE
     /\ z' = "s2_3"
     /\ pc' = <<6, "t", TRUE>>
     /\ msgs' = 53
     /\ Q' = TRUE
     /\ state' = "s6_3"
     /\ a1' = <<10, "t", TRUE>>
     /\ b_2' = 93
     /\ votes' = TRUE
     /\ rmState' = "s10_3"
     /\ tmPrepared' = <<14, "t", TRUE>>
     /\ AaAa' = 133
====
