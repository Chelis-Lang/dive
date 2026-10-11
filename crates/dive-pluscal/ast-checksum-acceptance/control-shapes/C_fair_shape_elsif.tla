---- MODULE C_fair_shape_elsif ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0; { L: if (x=0) { x:=1; } else { if (x=1) { x:=2; } else {skip;} } } } *)
====
