---- MODULE P_expression_member ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin L: x:= IF 0 ∈ {0} THEN 1 ELSE 0; end algorithm; *)
====
