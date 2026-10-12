---- MODULE C_fair_record_field ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:x:=1; y:=[x |-> x,
  z |-> x].x; E:skip; } } *)
====
