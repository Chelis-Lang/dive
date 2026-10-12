---- MODULE C_fair_await ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:await (y=0
  = TRUE); E:skip; } } *)
====
