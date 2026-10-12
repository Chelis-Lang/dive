---- MODULE C_macro_empty_parameters ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; macro M() {x:=1+
 2;} { L:M(); E:skip; } } *)
====
