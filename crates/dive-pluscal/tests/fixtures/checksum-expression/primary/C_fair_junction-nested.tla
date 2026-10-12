---- MODULE C_fair_junction_nested ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables x=0,y=0; { L:await /\ (y=0 \/ y=1)
      /\ TRUE; E:skip; } } *)
====
