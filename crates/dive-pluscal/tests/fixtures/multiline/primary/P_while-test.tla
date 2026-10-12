---- MODULE P_while_test ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:while x<2
  = TRUE do x:=x+1; end while; E:skip; end algorithm; *)
====
