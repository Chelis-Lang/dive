---- MODULE C_changed_read ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:	(* 😀 outside strings *) x:=1; y:=x +
	1; E:skip; } } *)
====
