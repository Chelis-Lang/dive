---- MODULE C_fair_shape_nested_if ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0; { L: if(x=0){if(x=1){x:=2;}else{x:=1;}}else{skip;} } } *)
====
