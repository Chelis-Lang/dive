---- MODULE C_case_declaration ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=CASE TRUE -> 1
  [] OTHER -> 0,y=0; { L:skip; E:skip; } } *)
====
