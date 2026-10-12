---- MODULE C_if_value ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:x:=IF y=0
  THEN 1
  ELSE 2; E:skip; } } *)
====
