---- MODULE C_case_value ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:x:=CASE y=0 -> 1
  [] OTHER -> 2; E:skip; } } *)
====
