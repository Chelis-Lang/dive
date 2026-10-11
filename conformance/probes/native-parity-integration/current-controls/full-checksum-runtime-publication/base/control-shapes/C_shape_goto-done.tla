---- MODULE C_shape_goto_done ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: x:=1; goto "Done"; } } *)
====
