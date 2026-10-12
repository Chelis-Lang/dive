---- MODULE P_fair_while_test_valid ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=0,y=0; begin L:while (x<2)
  = TRUE do x:=x+1; end while; E:skip; end algorithm; *)
====
