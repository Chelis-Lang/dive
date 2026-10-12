---- MODULE C_fair_if_first_line ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:x:=IF
  TRUE THEN 1 ELSE 0; E:skip; } } *)
====
