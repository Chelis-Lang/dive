---- MODULE CollideRev ----
EXTENDS Naturals
VARIABLES n, BBAa, AaBB, BBBB, AaAa
Init ==
  /\ n = 10
  /\ BBAa = FALSE
  /\ AaBB = "s2_0"
  /\ BBBB = <<3, "t", FALSE>>
  /\ AaAa = 50
Next ==
  \/ /\ n = 10
     /\ n' = 11
     /\ BBAa' = TRUE
     /\ AaBB' = "s2_1"
     /\ BBBB' = <<4, "t", TRUE>>
     /\ AaAa' = 51
  \/ /\ n = 11
     /\ n' = 12
     /\ BBAa' = FALSE
     /\ AaBB' = "s2_2"
     /\ BBBB' = <<5, "t", FALSE>>
     /\ AaAa' = 52
  \/ /\ n = 12
     /\ n' = 13
     /\ BBAa' = TRUE
     /\ AaBB' = "s2_3"
     /\ BBBB' = <<6, "t", TRUE>>
     /\ AaAa' = 53
====
