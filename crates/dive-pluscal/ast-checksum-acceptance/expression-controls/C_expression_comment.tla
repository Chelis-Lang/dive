---- MODULE C_expression_comment ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: x:= 1 (* inner *) + 2; } } *)
====
