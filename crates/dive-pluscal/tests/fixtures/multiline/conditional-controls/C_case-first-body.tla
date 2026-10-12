---- MODULE C_case_first_body ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:x:=CASE TRUE ->
  1 [] OTHER -> 0; E:skip; } } *)
====
