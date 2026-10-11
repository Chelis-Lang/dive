---- MODULE C_fixed_junction_or ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:await \/ FALSE \/ y=0; E:skip; } } *)
====
