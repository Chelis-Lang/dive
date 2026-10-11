---- MODULE C_fixed_junction_comment ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:await /\ TRUE (* inside *)
      /\ y=0; E:skip; } } *)
====
