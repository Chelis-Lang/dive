---- MODULE C_fair_changed_read ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:x:=1; y:=x +
  1; E:skip; } } *)
====
