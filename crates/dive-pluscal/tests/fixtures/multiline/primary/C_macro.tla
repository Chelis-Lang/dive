---- MODULE C_macro ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; macro M() begin x:=1+
 2; end macro; { L:M(); E:skip; } } *)
====
