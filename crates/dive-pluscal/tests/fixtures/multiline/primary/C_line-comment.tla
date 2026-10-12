---- MODULE C_line_comment ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:x:=1 + \* α comment
  2; E:skip; } } *)
====
