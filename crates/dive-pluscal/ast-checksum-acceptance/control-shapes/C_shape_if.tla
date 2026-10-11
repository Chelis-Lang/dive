---- MODULE C_shape_if ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: if (x=0) { x:=1; } else { skip; } } } *)
====
