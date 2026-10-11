---- MODULE C_fixed_junction_unicode ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:await ∧ TRUE
      ∧ y=0; E:skip; } } *)
====
