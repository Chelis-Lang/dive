---- MODULE C_fair_fixed_plus_line_comment ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:x:=1 + \* retained comment
      2; E:skip; } } *)
====
