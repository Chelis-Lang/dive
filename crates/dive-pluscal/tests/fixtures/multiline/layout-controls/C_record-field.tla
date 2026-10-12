---- MODULE C_record_field ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:	(* 😀 outside strings *) x:=1; y:=[x |-> x,
	z |-> x].x; E:skip; } } *)
====
