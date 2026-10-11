---- MODULE C_expression_choose ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: x:= CHOOSE z ∈ {1}: z=1; } } *)
====
