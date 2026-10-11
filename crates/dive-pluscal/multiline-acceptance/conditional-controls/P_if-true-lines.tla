---- MODULE P_if_true_lines ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:x:=IF TRUE
  THEN 1 ELSE 0; E:skip; end algorithm; *)
====
