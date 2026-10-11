---- MODULE C_malformed_junction ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:await /\ ; E:skip; } } *)
====
