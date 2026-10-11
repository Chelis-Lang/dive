---- MODULE C_expression_let ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: x:= LET z == 1 IN z; } } *)
====
