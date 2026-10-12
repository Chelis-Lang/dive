---- MODULE C_if_true_lines ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:x:=IF TRUE
  THEN 1 ELSE 0; E:skip; } } *)
====
