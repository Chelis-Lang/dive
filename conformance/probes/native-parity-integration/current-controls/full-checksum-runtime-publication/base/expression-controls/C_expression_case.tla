---- MODULE C_expression_case ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: x:= CASE TRUE -> 1 [] OTHER -> 0; } } *)
====
