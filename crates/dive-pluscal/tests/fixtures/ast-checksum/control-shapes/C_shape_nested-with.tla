---- MODULE C_shape_nested_with ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: with(a=1){with(b=2){x:=a+b;}} } } *)
====
