---- MODULE C_fair_while_test_valid ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:while((x<2)
  = TRUE){x:=x+1;}; E:skip; } } *)
====
