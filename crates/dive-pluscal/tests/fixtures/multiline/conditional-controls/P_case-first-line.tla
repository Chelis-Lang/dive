---- MODULE P_case_first_line ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:x:=CASE
  TRUE -> 1 [] OTHER -> 0; E:skip; end algorithm; *)
====
