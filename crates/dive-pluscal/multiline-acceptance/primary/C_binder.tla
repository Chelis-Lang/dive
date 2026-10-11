---- MODULE C_binder ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:x:=CHOOSE z \in {1,
  2}:TRUE; E:skip; } } *)
====
