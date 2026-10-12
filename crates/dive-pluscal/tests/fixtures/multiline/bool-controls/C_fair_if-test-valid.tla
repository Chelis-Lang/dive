---- MODULE C_fair_if_test_valid ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:if((y=0)
  = TRUE){x:=1;}else{x:=2;}; E:skip; } } *)
====
