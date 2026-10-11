---- MODULE P_if_test_valid ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:if (y=0)
  = TRUE then x:=1; else x:=2; end if; E:skip; end algorithm; *)
====
