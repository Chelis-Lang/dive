---- MODULE C_expression_conjunction ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: x:= IF TRUE ∧ FALSE THEN 1 ELSE 0; } } *)
====
