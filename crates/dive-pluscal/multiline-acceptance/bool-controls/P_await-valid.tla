---- MODULE P_await_valid ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:await (y=0)
  = TRUE; E:skip; end algorithm; *)
====
