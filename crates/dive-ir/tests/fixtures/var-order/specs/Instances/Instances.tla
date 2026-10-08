---- MODULE Instances ----
EXTENDS Naturals
VARIABLES x, y, BBBB, state
I == INSTANCE Inst
J == INSTANCE Inst WITH x <- y, y <- x
INSTANCE Inst
LOCAL INSTANCE Sequences
---- MODULE Sub ----
VARIABLE AaAa
S1 == AaAa
====
VARIABLES AaAa, msgs
Init ==
  /\ x = 10
  /\ y = FALSE
  /\ BBBB = "s2_0"
  /\ state = <<3, "t", FALSE>>
  /\ AaAa = 50
  /\ msgs = FALSE
Next ==
  \/ /\ x = 10
     /\ x' = 11
     /\ y' = TRUE
     /\ BBBB' = "s2_1"
     /\ state' = <<4, "t", TRUE>>
     /\ AaAa' = 51
     /\ msgs' = TRUE
  \/ /\ x = 11
     /\ x' = 12
     /\ y' = FALSE
     /\ BBBB' = "s2_2"
     /\ state' = <<5, "t", FALSE>>
     /\ AaAa' = 52
     /\ msgs' = FALSE
  \/ /\ x = 12
     /\ x' = 13
     /\ y' = TRUE
     /\ BBBB' = "s2_3"
     /\ state' = <<6, "t", TRUE>>
     /\ AaAa' = 53
     /\ msgs' = TRUE
====
