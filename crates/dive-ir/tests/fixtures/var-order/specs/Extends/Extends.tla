---- MODULE Extends ----
EXTENDS ExtB, ExtC, TLC
VARIABLES z, votes
Init ==
  /\ a1 = 10
  /\ pc = FALSE
  /\ msgs = "s2_0"
  /\ BBBB = <<3, "t", FALSE>>
  /\ AaAa = 50
  /\ q = FALSE
  /\ z = "s6_0"
  /\ votes = <<7, "t", FALSE>>
Next ==
  \/ /\ a1 = 10
     /\ a1' = 11
     /\ pc' = TRUE
     /\ msgs' = "s2_1"
     /\ BBBB' = <<4, "t", TRUE>>
     /\ AaAa' = 51
     /\ q' = TRUE
     /\ z' = "s6_1"
     /\ votes' = <<8, "t", TRUE>>
  \/ /\ a1 = 11
     /\ a1' = 12
     /\ pc' = FALSE
     /\ msgs' = "s2_2"
     /\ BBBB' = <<5, "t", FALSE>>
     /\ AaAa' = 52
     /\ q' = FALSE
     /\ z' = "s6_2"
     /\ votes' = <<9, "t", FALSE>>
  \/ /\ a1 = 12
     /\ a1' = 13
     /\ pc' = TRUE
     /\ msgs' = "s2_3"
     /\ BBBB' = <<6, "t", TRUE>>
     /\ AaAa' = 53
     /\ q' = TRUE
     /\ z' = "s6_3"
     /\ votes' = <<10, "t", TRUE>>
====
