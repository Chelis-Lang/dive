---- MODULE P_macro_empty_parameters ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; macro M() begin x:=1+
 2; end macro; begin L:M(); E:skip; end algorithm; *)
====
