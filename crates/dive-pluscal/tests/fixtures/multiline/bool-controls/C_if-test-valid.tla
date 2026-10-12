---- MODULE C_if_test_valid ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:if((y=0)
  = TRUE){x:=1;}else{x:=2;}; E:skip; } } *)
====
