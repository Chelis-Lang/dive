---- MODULE C_shape_while_mixed ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: while (x<2) {x:=x+1; B:skip;} } } *)
====
