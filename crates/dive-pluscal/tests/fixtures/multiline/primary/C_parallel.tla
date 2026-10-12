---- MODULE C_parallel ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:x:=y +
  1 || y:=x +
  2; E:skip; } } *)
====
