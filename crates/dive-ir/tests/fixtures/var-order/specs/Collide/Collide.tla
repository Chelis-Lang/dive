---- MODULE Collide ----
EXTENDS Naturals
VARIABLES n, AaAa, BBBB, AaBB, BBAa
Init ==
  /\ n = 10
  /\ AaAa = FALSE
  /\ BBBB = "s2_0"
  /\ AaBB = <<3, "t", FALSE>>
  /\ BBAa = 50
Next ==
  \/ /\ n = 10
     /\ n' = 11
     /\ AaAa' = TRUE
     /\ BBBB' = "s2_1"
     /\ AaBB' = <<4, "t", TRUE>>
     /\ BBAa' = 51
  \/ /\ n = 11
     /\ n' = 12
     /\ AaAa' = FALSE
     /\ BBBB' = "s2_2"
     /\ AaBB' = <<5, "t", FALSE>>
     /\ BBAa' = 52
  \/ /\ n = 12
     /\ n' = 13
     /\ AaAa' = TRUE
     /\ BBBB' = "s2_3"
     /\ AaBB' = <<6, "t", TRUE>>
     /\ BBAa' = 53
====
