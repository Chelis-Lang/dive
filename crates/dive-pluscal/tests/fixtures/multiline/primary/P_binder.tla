---- MODULE P_binder ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:x:=CHOOSE z \in {1,
  2}:TRUE; E:skip; end algorithm; *)
====
