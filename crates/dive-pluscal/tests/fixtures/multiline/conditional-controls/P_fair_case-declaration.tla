---- MODULE P_fair_case_declaration ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=CASE TRUE -> 1
  [] OTHER -> 0,y=0; begin L:skip; E:skip; end algorithm; *)
====
