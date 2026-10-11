---- MODULE P_expression_comment ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin L: x:= 1 (* inner *) + 2; end algorithm; *)
====
