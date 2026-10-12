---- MODULE P_fair_await_valid ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=0,y=0; begin L:await (y=0)
  = TRUE; E:skip; end algorithm; *)
====
